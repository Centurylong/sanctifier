use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects gas-wasting clones of the Soroban `Env` handle (`env.clone()`).
///
/// The `Env` is a cheap-to-pass host handle, but calling `.clone()` on it is a
/// common copy-paste habit that adds needless host work on every invocation.
/// Idiomatic Soroban passes `&env` (or `env.clone()` only when an owned handle
/// is genuinely required by an API). This rule flags the bare `env.clone()`
/// call so the author can pass a reference instead. To keep the signal high it
/// deliberately targets the `Env` handle only, not arbitrary value clones.
pub struct ExcessiveCloneRule;

impl ExcessiveCloneRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExcessiveCloneRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ExcessiveCloneRule {
    fn name(&self) -> &str {
        "excessive_clone"
    }

    fn description(&self) -> &str {
        "Detects gas-wasting `.clone()` of the Soroban Env handle where a reference would do"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = CloneVisitor {
            fn_name: String::new(),
            seen: HashSet::new(),
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct CloneVisitor {
    fn_name: String,
    seen: HashSet<usize>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for CloneVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev = std::mem::replace(&mut self.fn_name, node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.fn_name = prev;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev = std::mem::replace(&mut self.fn_name, node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.fn_name = prev;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "clone" && node.args.is_empty() && receiver_is_env(&node.receiver) {
            let line = node.span().start().line;
            if self.seen.insert(line) {
                self.violations.push(
                    RuleViolation::new(
                        "excessive_clone",
                        Severity::Warning,
                        "Gas-wasting `.clone()` of the Env handle; pass `&env` by reference instead"
                            .to_string(),
                        format!("{}:{}", self.fn_name, line),
                    )
                    .with_suggestion(
                        "Borrow the Env (`&env`) or accept `&Env` in the callee rather than \
                         cloning the host handle on every call"
                            .to_string(),
                    ),
                );
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Whether the receiver of a `.clone()` is the `Env` handle: a bare `env` path
/// or a field access ending in `.env` (e.g. `self.env`).
fn receiver_is_env(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(|i| i == "env").unwrap_or(false),
        syn::Expr::Field(f) => match &f.member {
            syn::Member::Named(name) => name == "env",
            syn::Member::Unnamed(_) => false,
        },
        syn::Expr::Paren(p) => receiver_is_env(&p.expr),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_env_clone() {
        let rule = ExcessiveCloneRule::new();
        let source = r#"
            impl Contract {
                pub fn go(env: Env) {
                    helper(env.clone());
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("Env"));
    }

    #[test]
    fn flags_self_env_clone() {
        let rule = ExcessiveCloneRule::new();
        let source = r#"
            impl Contract {
                pub fn go(&self) {
                    let e = self.env.clone();
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn ignores_non_env_clone() {
        let rule = ExcessiveCloneRule::new();
        let source = r#"
            impl Contract {
                pub fn go(env: Env, addr: Address) {
                    let a = addr.clone();
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn dedups_per_line() {
        let rule = ExcessiveCloneRule::new();
        let source = r#"
            fn f(env: Env) {
                let _ = (env.clone(), env.clone());
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }
}
