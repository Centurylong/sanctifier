use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_BPS_MAGIC_DENOMINATOR";
const BPS_DENOMINATOR: u64 = 10_000;

/// Detects hardcoded or suspicious basis-point denominators in financial math.
pub struct BpsMagicDenominatorRule;

impl BpsMagicDenominatorRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BpsMagicDenominatorRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for BpsMagicDenominatorRule {
    fn name(&self) -> &str {
        "bps_magic_denominator"
    }

    fn description(&self) -> &str {
        "Detects hardcoded or mismatched basis-point denominators in financial math"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = BpsMagicDenominatorVisitor {
            current_fn: None,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct BpsMagicDenominatorVisitor {
    current_fn: Option<String>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for BpsMagicDenominatorVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let previous = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.current_fn = previous;
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Div(_)) {
            if let Some(denominator) = int_literal(&node.right) {
                if is_bps_like_expr(&node.left) && is_problem_denominator(denominator) {
                    let fn_name = self.current_fn.as_deref().unwrap_or("<module>");
                    self.violations.push(
                        RuleViolation::new(
                            FINDING_CODE,
                            Severity::Warning,
                            message_for_denominator(denominator),
                            format!("{}:{}", fn_name, node.span().start().line),
                        )
                        .with_suggestion(
                            "Define and reuse a named constant such as `const BASIS_POINTS_DENOMINATOR: i128 = 10_000;` and validate any non-10_000 denominator against the intended unit"
                                .to_string(),
                        ),
                    );
                }
            }
        }

        syn::visit::visit_expr_binary(self, node);
    }
}

fn is_problem_denominator(value: u64) -> bool {
    value == BPS_DENOMINATOR || (1_000..=100_000).contains(&value)
}

fn message_for_denominator(value: u64) -> String {
    if value == BPS_DENOMINATOR {
        format!("{FINDING_CODE}: hardcoded basis-point denominator `10_000` should use a named constant")
    } else {
        format!(
            "{FINDING_CODE}: suspicious denominator `{value}` in bps-like math may not match the expected 10_000 basis-point scale"
        )
    }
}

fn is_bps_like_expr(expr: &syn::Expr) -> bool {
    let mut visitor = BpsIdentifierVisitor { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

struct BpsIdentifierVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for BpsIdentifierVisitor {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if is_bps_related_name(&ident.to_string()) {
            self.found = true;
        }
    }
}

fn is_bps_related_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "bps",
        "basis",
        "fee",
        "rate",
        "interest",
        "commission",
        "premium",
        "discount",
        "royalty",
        "spread",
        "slippage",
        "tax",
    ]
    .iter()
    .any(|token| lower == *token || lower.contains(token))
}

fn int_literal(expr: &syn::Expr) -> Option<u64> {
    match strip_wrappers(expr) {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => lit.base10_parse::<u64>().ok(),
        _ => None,
    }
}

fn strip_wrappers(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(paren) => strip_wrappers(&paren.expr),
        syn::Expr::Group(group) => strip_wrappers(&group.expr),
        _ => expr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_hardcoded_bps_denominator() {
        let source = r#"
            impl Contract {
                pub fn fee(amount: i128, fee_bps: i128) -> i128 {
                    amount * fee_bps / 10_000
                }
            }
        "#;

        let findings = BpsMagicDenominatorRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].message.contains("10_000"));
        assert!(findings[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("BASIS_POINTS_DENOMINATOR"));
    }

    #[test]
    fn flags_suspicious_bps_denominator() {
        let source = r#"
            impl Contract {
                pub fn interest(principal: i128, interest_rate: i128) -> i128 {
                    principal * interest_rate / 1_000
                }
            }
        "#;

        let findings = BpsMagicDenominatorRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("suspicious denominator"));
    }

    #[test]
    fn ignores_named_denominator_constant() {
        let source = r#"
            const BASIS_POINTS_DENOMINATOR: i128 = 10_000;

            impl Contract {
                pub fn fee(amount: i128, fee_bps: i128) -> i128 {
                    amount * fee_bps / BASIS_POINTS_DENOMINATOR
                }
            }
        "#;

        let findings = BpsMagicDenominatorRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_unrelated_numeric_division() {
        let source = r#"
            fn progress(done: i128, total: i128) -> i128 {
                done * 100 / total
            }
        "#;

        let findings = BpsMagicDenominatorRule::new().check(source);

        assert!(findings.is_empty());
    }
}
