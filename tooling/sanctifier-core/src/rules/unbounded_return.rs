use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File, Visibility};

/// Detects public entrypoints returning unbounded collections (`Vec` or `Map`).
pub struct UnboundedReturnRule;

impl UnboundedReturnRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnboundedReturnRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UnboundedReturnRule {
    fn name(&self) -> &str {
        "unbounded_return"
    }

    fn description(&self) -> &str {
        "Detects public entrypoints returning unbounded collections (Vec or Map)"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = UnboundedReturnVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct UnboundedReturnVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for UnboundedReturnVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        // Only analyze public methods
        if let Visibility::Public(_) = node.vis {
            let returns_collection = match &node.sig.output {
                syn::ReturnType::Type(_, ty) => {
                    let type_str = quote::quote!(#ty).to_string();
                    type_str.contains("Vec") || type_str.contains("Map")
                }
                _ => false,
            };

            if returns_collection {
                let has_pagination = node.sig.inputs.iter().any(|arg| {
                    if let syn::FnArg::Typed(pat) = arg {
                        let arg_str = quote::quote!(#pat).to_string();
                        arg_str.contains("limit") || arg_str.contains("offset") || arg_str.contains("start") || arg_str.contains("cursor") || arg_str.contains("page")
                    } else {
                        false
                    }
                });

                if !has_pagination {
                    self.violations.push(
                        RuleViolation::new(
                            "unbounded_return",
                            Severity::Warning,
                            "Unbounded collection (`Vec` or `Map`) returned to caller".to_string(),
                            format!("{}:{}", node.sig.ident, node.sig.ident.span().start().line),
                        )
                        .with_suggestion("Returning a storage-backed collection that grows with users can exceed return-size limits. Implement pagination using parameters like `start` and `limit`.".to_string()),
                    );
                }
            }
        }
        syn::visit::visit_impl_item_fn(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unbounded_vec_return() {
        let rule = UnboundedReturnRule::new();
        let source = r#"
            impl Contract {
                pub fn get_users(env: Env) -> Vec<Address> {
                    vec![]
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn allows_paginated_vec_return() {
        let rule = UnboundedReturnRule::new();
        let source = r#"
            impl Contract {
                pub fn get_users(env: Env, start: u32, limit: u32) -> Vec<Address> {
                    vec![]
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_private_functions() {
        let rule = UnboundedReturnRule::new();
        let source = r#"
            impl Contract {
                fn get_users_internal(env: Env) -> Vec<Address> {
                    vec![]
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }
}
