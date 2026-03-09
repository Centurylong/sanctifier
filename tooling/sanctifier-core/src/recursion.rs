use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use syn::visit::{self, Visit};
use syn::{parse_str, File, Item};

/// Represents a potential recursion issue that could exceed Soroban stack limits.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecursionIssue {
    /// The function that is part of a recursive call chain.
    pub function_name: String,
    /// Type of recursion detected: "direct", "indirect", or "potential".
    pub recursion_type: RecursionType,
    /// The call chain showing the recursion path (e.g., ["foo", "bar", "foo"]).
    pub call_chain: Vec<String>,
    /// Estimated maximum depth if detectable, otherwise None.
    pub estimated_depth: Option<usize>,
    /// Human-readable message describing the issue.
    pub message: String,
    /// Location context string.
    pub location: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RecursionType {
    /// Function calls itself directly.
    Direct,
    /// Function calls itself through one or more intermediate functions.
    Indirect,
    /// Function has recursive potential but may be bounded.
    Potential,
}

/// Analyzer for detecting recursive calls in Soroban contracts.
pub struct RecursionAnalyzer {
    /// Maps function names to the functions they call.
    call_graph: HashMap<String, HashSet<String>>,
    /// Tracks which functions are public contract methods.
    public_functions: HashSet<String>,
}

impl RecursionAnalyzer {
    pub fn new() -> Self {
        Self {
            call_graph: HashMap::new(),
            public_functions: HashSet::new(),
        }
    }

    /// Analyzes source code for recursive patterns.
    pub fn analyze(&mut self, source: &str) -> Vec<RecursionIssue> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        // Build call graph
        self.build_call_graph(&file);

        // Detect recursion patterns
        self.detect_recursion()
    }

    /// Builds a call graph by visiting all functions in the AST.
    fn build_call_graph(&mut self, file: &File) {
        for item in &file.items {
            if let Item::Impl(impl_item) = item {
                // Check if this is a contract impl
                let is_contract_impl = impl_item.attrs.iter().any(|attr| {
                    if let syn::Meta::Path(path) = &attr.meta {
                        path.is_ident("contractimpl")
                    } else {
                        false
                    }
                });

                for impl_fn in &impl_item.items {
                    if let syn::ImplItem::Fn(func) = impl_fn {
                        let fn_name = func.sig.ident.to_string();
                        
                        // Track public functions
                        if matches!(func.vis, syn::Visibility::Public(_)) && is_contract_impl {
                            self.public_functions.insert(fn_name.clone());
                        }

                        // Build call graph for this function
                        let mut visitor = CallGraphVisitor::new(fn_name.clone());
                        visitor.visit_impl_item_fn(func);
                        
                        self.call_graph.insert(fn_name, visitor.called_functions);
                    }
                }
            } else if let Item::Fn(func) = item {
                // Handle standalone functions
                let fn_name = func.sig.ident.to_string();
                let mut visitor = CallGraphVisitor::new(fn_name.clone());
                visitor.visit_item_fn(func);
                
                self.call_graph.insert(fn_name, visitor.called_functions);
            }
        }
    }

    /// Detects recursion patterns in the call graph.
    fn detect_recursion(&self) -> Vec<RecursionIssue> {
        let mut issues = Vec::new();
        let mut visited = HashSet::new();

        for fn_name in self.call_graph.keys() {
            if visited.contains(fn_name) {
                continue;
            }

            // Check for direct recursion
            if let Some(callees) = self.call_graph.get(fn_name) {
                if callees.contains(fn_name) {
                    issues.push(RecursionIssue {
                        function_name: fn_name.clone(),
                        recursion_type: RecursionType::Direct,
                        call_chain: vec![fn_name.clone(), fn_name.clone()],
                        estimated_depth: None,
                        message: format!(
                            "Function '{}' calls itself directly, which may exceed Soroban stack limits",
                            fn_name
                        ),
                        location: fn_name.clone(),
                    });
                    visited.insert(fn_name.clone());
                    continue;
                }
            }

            // Check for indirect recursion using DFS
            let mut path = vec![fn_name.clone()];
            let mut stack_visited = HashSet::new();
            
            if let Some(cycle) = self.find_cycle(fn_name, &mut path, &mut stack_visited) {
                let recursion_type = if cycle.len() == 2 {
                    RecursionType::Direct
                } else {
                    RecursionType::Indirect
                };

                issues.push(RecursionIssue {
                    function_name: fn_name.clone(),
                    recursion_type,
                    call_chain: cycle.clone(),
                    estimated_depth: None,
                    message: format!(
                        "Function '{}' is part of a recursive call chain: {}",
                        fn_name,
                        cycle.join(" -> ")
                    ),
                    location: fn_name.clone(),
                });

                // Mark all functions in the cycle as visited
                for func in &cycle {
                    visited.insert(func.clone());
                }
            }
        }

        issues
    }

    /// Finds a cycle in the call graph using DFS.
    /// Returns the cycle path if found, None otherwise.
    fn find_cycle(
        &self,
        current: &str,
        path: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) -> Option<Vec<String>> {
        if visited.contains(current) {
            // Found a cycle - extract it from the path
            if let Some(cycle_start) = path.iter().position(|f| f == current) {
                let mut cycle = path[cycle_start..].to_vec();
                cycle.push(current.to_string());
                return Some(cycle);
            }
            return None;
        }

        visited.insert(current.to_string());

        if let Some(callees) = self.call_graph.get(current) {
            for callee in callees {
                path.push(callee.clone());
                if let Some(cycle) = self.find_cycle(callee, path, visited) {
                    return Some(cycle);
                }
                path.pop();
            }
        }

        visited.remove(current);
        None
    }
}

impl Default for RecursionAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Visitor that collects all function calls within a function body.
struct CallGraphVisitor {
    current_function: String,
    called_functions: HashSet<String>,
}

impl CallGraphVisitor {
    fn new(current_function: String) -> Self {
        Self {
            current_function,
            called_functions: HashSet::new(),
        }
    }
}

impl<'ast> Visit<'ast> for CallGraphVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        // Handle direct function calls: foo()
        if let syn::Expr::Path(path_expr) = &*node.func {
            if let Some(segment) = path_expr.path.segments.last() {
                let fn_name = segment.ident.to_string();
                self.called_functions.insert(fn_name);
            }
        }
        
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        // Handle method calls: obj.method()
        let method_name = node.method.to_string();
        self.called_functions.insert(method_name);
        
        visit::visit_expr_method_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_direct_recursion() {
        let source = r#"
            impl MyContract {
                pub fn factorial(n: u32) -> u32 {
                    if n <= 1 {
                        1
                    } else {
                        n * factorial(n - 1)
                    }
                }
            }
        "#;

        let mut analyzer = RecursionAnalyzer::new();
        let issues = analyzer.analyze(source);
        
        assert!(!issues.is_empty());
        assert_eq!(issues[0].recursion_type, RecursionType::Direct);
        assert!(issues[0].message.contains("factorial"));
    }

    #[test]
    fn test_indirect_recursion() {
        let source = r#"
            impl MyContract {
                pub fn foo() {
                    bar();
                }
                
                fn bar() {
                    baz();
                }
                
                fn baz() {
                    foo();
                }
            }
        "#;

        let mut analyzer = RecursionAnalyzer::new();
        let issues = analyzer.analyze(source);
        
        assert!(!issues.is_empty());
        assert_eq!(issues[0].recursion_type, RecursionType::Indirect);
    }

    #[test]
    fn test_no_recursion() {
        let source = r#"
            impl MyContract {
                pub fn add(a: u32, b: u32) -> u32 {
                    a + b
                }
                
                pub fn multiply(a: u32, b: u32) -> u32 {
                    let result = add(a, b);
                    result * 2
                }
            }
        "#;

        let mut analyzer = RecursionAnalyzer::new();
        let issues = analyzer.analyze(source);
        
        assert!(issues.is_empty());
    }
}
