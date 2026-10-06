use crate::finding_codes::SELF_FLOW_GUARD;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::visit::Visit;


/// Detects transfer/referral entrypoints whose endpoint parameters are not
/// compared before the function continues.
pub struct SelfFlowGuardRule;

impl SelfFlowGuardRule {
    pub fn new() -> Self {
        Self
    }

    fn pair_for(sig: &syn::Signature) -> Option<(&'static str, &'static str)> {
        let name = sig.ident.to_string().to_ascii_lowercase();
        let transfer_pairs: &[(&str, &str)] = &[
            ("sender", "recipient"),
            ("sender", "receiver"),
            ("source", "destination"),
        ];
        let referral_pairs: &[(&str, &str)] = &[
            ("referrer", "referee"),
            ("referrer", "referred"),
        ];

        if name.contains("transfer") {
            if name.contains("from")
                && Self::has_param(sig, "from")
                && Self::has_param(sig, "to")
            {
                return Some(("from", "to"));
            }
            return transfer_pairs
                .iter()
                .copied()
                .find(|(left, right)| Self::has_param(sig, left) && Self::has_param(sig, right));
        }

        if name.contains("refer") {
            return referral_pairs
                .iter()
                .copied()
                .find(|(left, right)| Self::has_param(sig, left) && Self::has_param(sig, right));
        }

        None
    }

    fn has_param(sig: &syn::Signature, name: &str) -> bool {
        sig.inputs.iter().any(|input| {
            let syn::FnArg::Typed(input) = input else {
                return false;
            };
            let syn::Pat::Ident(ident) = &*input.pat else {
                return false;
            };
            ident.ident == name
        })
    }

    fn has_guard(block: &syn::Block, pair: (&str, &str)) -> bool {
        let mut visitor = GuardVisitor {
            left: pair.0,
            right: pair.1,
            found: false,
        };
        visitor.visit_block(block);
        visitor.found
    }

    fn check_fn(&self, sig: &syn::Signature, block: &syn::Block) -> Option<RuleViolation> {
        let pair = Self::pair_for(sig)?;
        if Self::has_guard(block, pair) {
            return None;
        }

        let name = sig.ident.to_string();
        Some(
            RuleViolation::new(
                SELF_FLOW_GUARD,
                Severity::Warning,
                format!(
                    "{SELF_FLOW_GUARD}: {name} accepts {} and {} without an explicit equality guard",
                    pair.0, pair.1
                ),
                format!("{}:{}", name, sig.span().start().line),
            )
            .with_suggestion(format!(
                "Compare {} and {} before applying the value-flow state change",
                pair.0, pair.1
            )),
        )
    }
}

impl Default for SelfFlowGuardRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for SelfFlowGuardRule {
    fn name(&self) -> &str {
        "self_flow_guard"
    }

    fn description(&self) -> &str {
        "Detects transfer/referral endpoint pairs without an explicit equality guard"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return vec![],
        };

        let mut visitor = FlowVisitor {
            rule: self,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FlowVisitor<'a> {
    rule: &'a SelfFlowGuardRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FlowVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if let Some(violation) = self.rule.check_fn(&node.sig, &node.block) {
            self.violations.push(violation);
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if let Some(violation) = self.rule.check_fn(&node.sig, &node.block) {
            self.violations.push(violation);
        }
        syn::visit::visit_impl_item_fn(self, node);
    }
}

struct GuardVisitor<'a> {
    left: &'a str,
    right: &'a str,
    found: bool,
}

impl GuardVisitor<'_> {
    fn expr_is_ident(expr: &syn::Expr, name: &str) -> bool {
        match expr {
            syn::Expr::Path(path) => path
                .path
                .segments
                .last()
                .map(|segment| segment.ident == name)
                .unwrap_or(false),
            syn::Expr::Reference(reference) => Self::expr_is_ident(&reference.expr, name),
            syn::Expr::Paren(paren) => Self::expr_is_ident(&paren.expr, name),
            syn::Expr::Group(group) => Self::expr_is_ident(&group.expr, name),
            syn::Expr::MethodCall(call)
                if call.method.to_string() == "clone" && call.args.is_empty() =>
            {
                Self::expr_is_ident(&call.receiver, name)
            }
            _ => false,
        }
    }

    fn matches_pair(&self, left: &syn::Expr, right: &syn::Expr) -> bool {
        (Self::expr_is_ident(left, self.left) && Self::expr_is_ident(right, self.right))
            || (Self::expr_is_ident(left, self.right)
                && Self::expr_is_ident(right, self.left))
    }
}

impl<'ast> Visit<'ast> for GuardVisitor<'_> {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_))
            && self.matches_pair(&node.left, &node.right)
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        let name = node
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .unwrap_or_default();
        if matches!(
            name.as_str(),
            "assert_eq" | "assert_ne" | "debug_assert_eq" | "debug_assert_ne"
        ) {
            if let Ok(args) =
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                    .parse2(node.tokens.clone())
            {
                let mut args = args.iter();
                if let (Some(left), Some(right)) = (args.next(), args.next()) {
                    if self.matches_pair(left, right) {
                        self.found = true;
                        return;
                    }
                }
            }
        }
        syn::visit::visit_macro(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_referral_pair_without_guard() {
        let source = r#"
            impl Rewards {
                pub fn reward_referral(referrer: Address, referee: Address) {
                    record(referrer, referee);
                }
            }
        "#;
        let findings = SelfFlowGuardRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, SELF_FLOW_GUARD);
    }

    #[test]
    fn recognizes_referral_equality_guard() {
        let source = r#"
            impl Rewards {
                pub fn reward_referral(referrer: Address, referee: Address) {
                    if referrer == referee {
                        return;
                    }
                    record(referrer, referee);
                }
            }
        "#;
        assert!(SelfFlowGuardRule::new().check(source).is_empty());
    }

    #[test]
    fn recognizes_assert_ne_and_transfer_aliases() {
        let source = r#"
            fn transfer_value(sender: Address, recipient: Address) {
                assert_ne!(sender, recipient);
                record(sender, recipient);
            }
        "#;
        assert!(SelfFlowGuardRule::new().check(source).is_empty());
    }

    #[test]
    fn macro_guard_requires_exact_endpoint_expressions() {
        let source = r#"
            fn reward_referral(referrer: Address, referee: Address) {
                let referrer_count = 1;
                let referee_count = 2;
                assert_ne!(referrer_count, referee_count);
                record(referrer, referee);
            }
        "#;
        let findings = SelfFlowGuardRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, SELF_FLOW_GUARD);
    }

    #[test]
    fn covers_transfer_from_without_duplicate_normal_transfer_detection() {
        let source = r#"
            impl Token {
                pub fn transfer_from(from: Address, to: Address, amount: i128) {
                    record(from, to, amount);
                }

                pub fn transfer(from: Address, to: Address, amount: i128) {
                    record(from, to, amount);
                }
            }
        "#;
        let findings = SelfFlowGuardRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].location.starts_with("transfer_from:"));
    }

    #[test]
    fn insta_snapshot_uncovered_endpoint_shapes() {
        let source = r#"
            impl Example {
                pub fn transfer_value(sender: Address, recipient: Address) {
                    record(sender, recipient);
                }

                pub fn reward_referral(referrer: Address, referee: Address) {
                    record(referrer, referee);
                }
            }
        "#;
        let findings = SelfFlowGuardRule::new().check(source);
        let snapshot: Vec<(&str, &str)> = findings
            .iter()
            .map(|finding| (finding.rule_name.as_str(), finding.message.as_str()))
            .collect();

        insta::assert_debug_snapshot!(snapshot, @r###"
        [
            (
                "SANCT_SELF_FLOW_GUARD",
                "SANCT_SELF_FLOW_GUARD: transfer_value accepts sender and recipient without an explicit equality guard",
            ),
            (
                "SANCT_SELF_FLOW_GUARD",
                "SANCT_SELF_FLOW_GUARD: reward_referral accepts referrer and referee without an explicit equality guard",
            ),
        ]
        "###);
    }
}