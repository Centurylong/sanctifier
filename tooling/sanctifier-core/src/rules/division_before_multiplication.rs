use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_DIV_BEFORE_MUL";

/// Detects money-like integer math that divides before multiplying.
pub struct DivisionBeforeMultiplicationRule;

impl DivisionBeforeMultiplicationRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DivisionBeforeMultiplicationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DivisionBeforeMultiplicationRule {
    fn name(&self) -> &str {
        "division_before_multiplication"
    }

    fn description(&self) -> &str {
        "Detects financial integer math that divides before multiplying and loses precision"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = DivisionBeforeMultiplicationVisitor {
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

struct DivisionBeforeMultiplicationVisitor {
    current_fn: Option<String>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for DivisionBeforeMultiplicationVisitor {
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
        if matches!(node.op, syn::BinOp::Mul(_)) && contains_division_operand(node) {
            if money_related_expr(&node.left) || money_related_expr(&node.right) {
                let fn_name = self.current_fn.as_deref().unwrap_or("<module>");
                self.violations.push(
                    RuleViolation::new(
                        FINDING_CODE,
                        Severity::Warning,
                        format!(
                            "{FINDING_CODE}: division-before-multiplication in money-like integer math can truncate before scaling"
                        ),
                        format!("{}:{}", fn_name, node.span().start().line),
                    )
                    .with_suggestion(
                        "Prefer overflow-safe multiply-first math such as `amount.checked_mul(rate)?.checked_div(denominator)?`, or a dedicated mul-div helper"
                            .to_string(),
                    ),
                );
            }
        }

        syn::visit::visit_expr_binary(self, node);
    }
}

fn contains_division_operand(node: &syn::ExprBinary) -> bool {
    expr_is_division(&node.left) || expr_is_division(&node.right)
}

fn expr_is_division(expr: &syn::Expr) -> bool {
    match strip_wrappers(expr) {
        syn::Expr::Binary(binary) => matches!(binary.op, syn::BinOp::Div(_)),
        _ => false,
    }
}

fn money_related_expr(expr: &syn::Expr) -> bool {
    let mut visitor = MoneyIdentifierVisitor { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

struct MoneyIdentifierVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for MoneyIdentifierVisitor {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if is_money_related_name(&ident.to_string()) {
            self.found = true;
        }
    }
}

fn is_money_related_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "amount",
        "balance",
        "reserve",
        "treasury",
        "price",
        "fee",
        "rate",
        "bps",
        "interest",
        "share",
        "supply",
        "value",
        "fund",
        "payout",
        "reward",
        "token",
        "asset",
        "liquidity",
        "principal",
        "collateral",
        "premium",
        "commission",
    ]
    .iter()
    .any(|token| lower == *token || lower.contains(token))
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
    fn flags_amount_divided_before_scaling() {
        let source = r#"
            impl Contract {
                pub fn payout(amount: i128, total_shares: i128, pool_balance: i128) -> i128 {
                    amount / total_shares * pool_balance
                }
            }
        "#;

        let findings = DivisionBeforeMultiplicationRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("checked_mul"));
    }

    #[test]
    fn flags_parenthesized_financial_division() {
        let source = r#"
            impl Contract {
                pub fn fee(balance: i128, denominator: i128, rate_bps: i128) -> i128 {
                    (balance / denominator) * rate_bps
                }
            }
        "#;

        let findings = DivisionBeforeMultiplicationRule::new().check(source);

        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_mul_before_division() {
        let source = r#"
            impl Contract {
                pub fn fee(amount: i128, rate_bps: i128, denominator: i128) -> i128 {
                    amount * rate_bps / denominator
                }
            }
        "#;

        let findings = DivisionBeforeMultiplicationRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_unrelated_integer_math() {
        let source = r#"
            fn layout(width: i128, columns: i128, rows: i128) -> i128 {
                width / columns * rows
            }
        "#;

        let findings = DivisionBeforeMultiplicationRule::new().check(source);

        assert!(findings.is_empty());
    }
}
