use quote::ToTokens;
use serde::{Deserialize, Serialize};
use syn::spanned::Spanned;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PathNode {
    pub node_type: String, // "condition" | "operation" | "panic"
    pub description: String,
    pub line: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExecutionPath {
    pub nodes: Vec<PathNode>,
    pub is_panic: bool,
}

impl Default for ExecutionPath {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionPath {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            is_panic: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SymbolicGraph {
    pub function_name: String,
    pub paths: Vec<ExecutionPath>,
}

pub struct SymbolicAnalyzer;

impl SymbolicAnalyzer {
    pub fn analyze_function(fn_item: &syn::ImplItemFn) -> SymbolicGraph {
        let fn_name = fn_item.sig.ident.to_string();
        let initial_paths = vec![ExecutionPath::new()];
        let final_paths = Self::traverse_stmts(&fn_item.block.stmts, initial_paths);

        SymbolicGraph {
            function_name: fn_name,
            paths: final_paths,
        }
    }

    fn traverse_stmts(stmts: &[syn::Stmt], base_paths: Vec<ExecutionPath>) -> Vec<ExecutionPath> {
        let mut current_paths = base_paths;

        for stmt in stmts {
            let mut next_paths = Vec::new();
            for path in current_paths {
                if path.is_panic {
                    next_paths.push(path);
                    continue; // Skip further statements in a panicked path
                }

                let branched = Self::explore_stmt(stmt, path);
                next_paths.extend(branched);
            }
            current_paths = next_paths;
        }

        current_paths
    }

    fn explore_stmt(stmt: &syn::Stmt, mut path: ExecutionPath) -> Vec<ExecutionPath> {
        match stmt {
            syn::Stmt::Local(local) => {
                let init_expr = local.init.as_ref().map(|init| &init.expr);
                if let Some(expr) = init_expr {
                    Self::explore_expr(expr, path)
                } else {
                    vec![path]
                }
            }
            syn::Stmt::Expr(expr, _) => Self::explore_expr(expr, path),
            syn::Stmt::Macro(m) => {
                if m.mac.path.is_ident("panic") {
                    path.nodes.push(PathNode {
                        node_type: "panic".to_string(),
                        description: "panic!(...)".to_string(),
                        line: m.mac.span().start().line,
                    });
                    path.is_panic = true;
                } else {
                    path.nodes.push(PathNode {
                        node_type: "operation".to_string(),
                        description: format!("Macro: {}", m.mac.path.to_token_stream()),
                        line: m.mac.span().start().line,
                    });
                }
                vec![path]
            }
            syn::Stmt::Item(_) => vec![path],
        }
    }

    fn explore_expr(expr: &syn::Expr, mut path: ExecutionPath) -> Vec<ExecutionPath> {
        match expr {
            syn::Expr::Block(b) => Self::traverse_stmts(&b.block.stmts, vec![path]),
            syn::Expr::If(if_expr) => {
                let mut paths = Vec::new();

                // True branch
                let mut true_path = path.clone();
                true_path.nodes.push(PathNode {
                    node_type: "condition".to_string(),
                    description: format!("If ({})", if_expr.cond.to_token_stream()),
                    line: if_expr.cond.span().start().line,
                });
                paths.extend(Self::traverse_stmts(
                    &if_expr.then_branch.stmts,
                    vec![true_path],
                ));

                // False branch
                let mut false_path = path;
                false_path.nodes.push(PathNode {
                    node_type: "condition".to_string(),
                    description: format!("Else (!{})", if_expr.cond.to_token_stream()),
                    line: if_expr.cond.span().start().line,
                });

                if let Some((_, else_branch)) = &if_expr.else_branch {
                    paths.extend(Self::explore_expr(else_branch, false_path));
                } else {
                    // Implicit empty else block
                    paths.push(false_path);
                }

                paths
            }
            syn::Expr::Match(match_expr) => {
                let mut paths = Vec::new();
                for arm in &match_expr.arms {
                    let mut arm_path = path.clone();
                    arm_path.nodes.push(PathNode {
                        node_type: "condition".to_string(),
                        description: format!("Match branch ({})", arm.pat.to_token_stream()),
                        line: arm.pat.span().start().line,
                    });
                    paths.extend(Self::explore_expr(&arm.body, arm_path));
                }
                paths
            }
            syn::Expr::Call(call_expr) => {
                path.nodes.push(PathNode {
                    node_type: "operation".to_string(),
                    description: format!("Call {}", call_expr.func.to_token_stream()),
                    line: call_expr.span().start().line,
                });
                vec![path]
            }
            syn::Expr::MethodCall(method_expr) => {
                let method_name = method_expr.method.to_string();
                if method_name == "unwrap" || method_name == "expect" {
                    path.nodes.push(PathNode {
                        node_type: "panic".to_string(),
                        description: format!(".{}() call", method_name),
                        line: method_expr.span().start().line,
                    });
                    path.is_panic = true;
                    vec![path]
                } else {
                    path.nodes.push(PathNode {
                        node_type: "operation".to_string(),
                        description: format!("Method {}", method_name),
                        line: method_expr.span().start().line,
                    });
                    vec![path]
                }
            }
            syn::Expr::Macro(m) => {
                if m.mac.path.is_ident("panic") {
                    path.nodes.push(PathNode {
                        node_type: "panic".to_string(),
                        description: "panic!(...)".to_string(),
                        line: m.mac.span().start().line,
                    });
                    path.is_panic = true;
                } else {
                    path.nodes.push(PathNode {
                        node_type: "operation".to_string(),
                        description: format!("Macro: {}", m.mac.path.to_token_stream()),
                        line: m.mac.span().start().line,
                    });
                }
                vec![path]
            }

            // Standard assignment / unary / binary math operations, we just record them.
            syn::Expr::Assign(assign_expr) => {
                path.nodes.push(PathNode {
                    node_type: "operation".to_string(),
                    description: "Assignment".to_string(),
                    line: assign_expr.span().start().line,
                });
                vec![path]
            }

            _ => {
                // Return unchanged for expressions that do not branch
                vec![path]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_str;

    #[test]
    fn test_linear_path() {
        let code = r#"
            pub fn simple_fn() {
                let x = 1;
                let y = 2;
                let z = x + y;
            }
        "#;
        let fn_item: syn::ImplItemFn = parse_str(code).unwrap();
        let graph = SymbolicAnalyzer::analyze_function(&fn_item);

        assert_eq!(graph.function_name, "simple_fn");
        assert_eq!(graph.paths.len(), 1);
        assert!(!graph.paths[0].is_panic);
    }

    #[test]
    fn test_branching_path() {
        let code = r#"
            pub fn branch_fn(flag: bool) {
                if flag {
                    let a = 1;
                } else {
                    let b = 2;
                }
            }
        "#;
        let fn_item: syn::ImplItemFn = parse_str(code).unwrap();
        let graph = SymbolicAnalyzer::analyze_function(&fn_item);

        assert_eq!(graph.paths.len(), 2);
    }

    #[test]
    fn test_panic_path() {
        let code = r#"
            pub fn panic_fn() {
                let x = Some(1).unwrap();
            }
        "#;
        let fn_item: syn::ImplItemFn = parse_str(code).unwrap();
        let graph = SymbolicAnalyzer::analyze_function(&fn_item);

        assert_eq!(graph.paths.len(), 1);
        assert!(graph.paths[0].is_panic);
    }
}
