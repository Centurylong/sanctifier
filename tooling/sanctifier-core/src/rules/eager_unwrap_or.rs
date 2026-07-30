use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Expr, File};

/// Detects eagerly-computed expensive defaults in `unwrap_or()`.
pub struct EagerUnwrapOrRule;

impl EagerUnwrapOrRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EagerUnwrapOrRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for EagerUnwrapOrRule {
    fn name(&self) -> &str {
        "eager_unwrap_or"
    }

    fn description(&self) -> &str {
        "Detects eagerly-computed expensive defaults in unwrap_or() which waste gas"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = EagerUnwrapOrVisitor {
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

struct EagerUnwrapOrVisitor {
    fn_name: String,
    seen: HashSet<usize>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for EagerUnwrapOrVisitor {
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
        if node.method == "unwrap_or" {
            if let Some(arg) = node.args.first() {
                let is_expensive =
                    matches!(arg, Expr::Call(_) | Expr::MethodCall(_) | Expr::Macro(_));

                if is_expensive {
                    let line = node.span().start().line;
                    if self.seen.insert(line) {
                        self.violations.push(
                            RuleViolation::new(
                                "eager_unwrap_or",
                                Severity::Warning,
                                "Eagerly-computed expensive fallback in `unwrap_or()` wastes gas on the hit path".to_string(),
                                format!("{}:{}", self.fn_name, line),
                            )
                            .with_suggestion("Use `unwrap_or_else(|| ...)` to evaluate the fallback lazily only when needed".to_string()),
                        );
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_expensive_call() {
        let rule = EagerUnwrapOrRule::new();
        let source = r#"
            fn test() {
                let x = Some(5);
                x.unwrap_or(compute_expensive());
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn ignores_cheap_literal() {
        let rule = EagerUnwrapOrRule::new();
        let source = r#"
            fn test() {
                let x = Some(5);
                x.unwrap_or(0);
                x.unwrap_or(true);
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }
}
