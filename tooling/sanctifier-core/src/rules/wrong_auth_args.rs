use crate::rules::{Patch, Rule, RuleViolation, Severity};
use syn::visit::Visit;

pub struct WrongAuthArgsRule;

impl WrongAuthArgsRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WrongAuthArgsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for WrongAuthArgsRule {
    fn name(&self) -> &str {
        "wrong_auth_args"
    }

    fn description(&self) -> &str {
        "Detects require_auth() inside non-public functions, which fails to bind specific internal arguments."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(f) => (*f).clone(),
            None => return vec![],
        };

        let mut visitor = AuthVisitor {
            violations: Vec::new(),
            rule_name: self.name(),
        };
        visitor.visit_file(&file);

        visitor.violations
    }

    fn fix(&self, _source: &str) -> Vec<Patch> {
        // Safe auto-fixing is difficult because we don't know the exact args to bind.
        vec![]
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct AuthVisitor<'a> {
    violations: Vec<RuleViolation>,
    rule_name: &'a str,
}

impl<'a> Visit<'a> for AuthVisitor<'a> {
    fn visit_item_fn(&mut self, node: &'a syn::ItemFn) {
        // If the function is not public, check its body for require_auth
        if !is_public(&node.vis) {
            let mut checker = RequireAuthChecker { found: false };
            checker.visit_block(&node.block);
            if checker.found {
                self.violations.push(
                    RuleViolation::new(
                        self.rule_name,
                        Severity::Error,
                        format!("Internal function '{}' uses require_auth() which bounds to top-level contract arguments", node.sig.ident),
                        node.sig.ident.to_string(),
                    )
                    .with_suggestion("Use require_auth_for_args() instead to bind this internal function's specific arguments".to_string()),
                );
            }
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'a syn::ImplItemFn) {
        // If the function is not public, check its body for require_auth
        if !is_public(&node.vis) {
            let mut checker = RequireAuthChecker { found: false };
            checker.visit_block(&node.block);
            if checker.found {
                self.violations.push(
                    RuleViolation::new(
                        self.rule_name,
                        Severity::Error,
                        format!("Internal function '{}' uses require_auth() which bounds to top-level contract arguments", node.sig.ident),
                        node.sig.ident.to_string(),
                    )
                    .with_suggestion("Use require_auth_for_args() instead to bind this internal function's specific arguments".to_string()),
                );
            }
        }
        syn::visit::visit_impl_item_fn(self, node);
    }
}

fn is_public(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

struct RequireAuthChecker {
    found: bool,
}

impl<'a> Visit<'a> for RequireAuthChecker {
    fn visit_expr_method_call(&mut self, node: &'a syn::ExprMethodCall) {
        if node.method == "require_auth" {
            self.found = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}
