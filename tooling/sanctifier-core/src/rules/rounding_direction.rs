use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::BTreeSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_ROUNDING_DIRECTION";

/// Detects inconsistent rounding direction across related financial math.
pub struct RoundingDirectionRule;

impl RoundingDirectionRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RoundingDirectionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for RoundingDirectionRule {
    fn name(&self) -> &str {
        "rounding_direction"
    }

    fn description(&self) -> &str {
        "Detects inconsistent rounding direction across related financial math"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = RoundingDirectionRuleVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct RoundingDirectionRuleVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for RoundingDirectionRuleVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations
            .extend(check_function(&node.sig.ident.to_string(), &node.block));
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations
            .extend(check_function(&node.sig.ident.to_string(), &node.block));
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        let mut related = Vec::new();
        for item in &node.items {
            let syn::ImplItem::Fn(method) = item else {
                continue;
            };

            let fn_name = method.sig.ident.to_string();
            if !is_financial_name(&fn_name) {
                continue;
            }

            let summary = rounding_summary(&method.block);
            if summary.is_empty() || has_conflicting_directions(&summary) {
                continue;
            }

            related.push((fn_name, summary));
        }

        let directions: BTreeSet<RoundingDirection> = related
            .iter()
            .flat_map(|(_, summary)| summary.iter().copied())
            .collect();

        if directions.len() > 1 {
            let function_list = related
                .iter()
                .map(|(fn_name, summary)| {
                    format!("`{fn_name}` uses {}", format_directions(summary))
                })
                .collect::<Vec<_>>()
                .join(", ");

            self.violations.push(
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    format!(
                        "{FINDING_CODE}: related financial functions use inconsistent rounding directions: {function_list}"
                    ),
                    format!("impl:{}", node.impl_token.span().start().line),
                )
                .with_suggestion(safe_rounding_suggestion()),
            );
        }

        syn::visit::visit_item_impl(self, node);
    }
}

fn check_function(fn_name: &str, block: &syn::Block) -> Vec<RuleViolation> {
    let summary = rounding_summary(block);
    if !has_conflicting_directions(&summary) || !is_financial_name(fn_name) {
        return Vec::new();
    }

    vec![RuleViolation::new(
        FINDING_CODE,
        Severity::Warning,
        format!(
            "{FINDING_CODE}: financial function `{fn_name}` mixes rounding directions ({})",
            format_directions(&summary)
        ),
        format!("{}:{}", fn_name, block.brace_token.span.open().start().line),
    )
    .with_suggestion(safe_rounding_suggestion())]
}

fn rounding_summary(block: &syn::Block) -> BTreeSet<RoundingDirection> {
    let mut visitor = RoundingVisitor {
        directions: BTreeSet::new(),
    };
    visitor.visit_block(block);
    visitor.directions
}

struct RoundingVisitor {
    directions: BTreeSet<RoundingDirection>,
}

impl<'ast> Visit<'ast> for RoundingVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(direction) = classify_rounding_name(&node.method.to_string()) {
            self.directions.insert(direction);
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref() {
            if let Some(segment) = path.path.segments.last() {
                if let Some(direction) = classify_rounding_name(&segment.ident.to_string()) {
                    self.directions.insert(direction);
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RoundingDirection {
    Down,
    Nearest,
    Up,
}

fn has_conflicting_directions(summary: &BTreeSet<RoundingDirection>) -> bool {
    summary.len() > 1
}

fn classify_rounding_name(name: &str) -> Option<RoundingDirection> {
    let lower = name.to_ascii_lowercase();
    if lower.contains("ceil") || lower.contains("round_up") || lower.contains("roundup") {
        return Some(RoundingDirection::Up);
    }
    if lower.contains("floor")
        || lower.contains("round_down")
        || lower.contains("rounddown")
        || lower.contains("trunc")
    {
        return Some(RoundingDirection::Down);
    }
    if lower == "round" || lower.contains("round_nearest") || lower.contains("round_to_nearest") {
        return Some(RoundingDirection::Nearest);
    }
    None
}

fn is_financial_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "deposit",
        "withdraw",
        "redeem",
        "claim",
        "mint",
        "burn",
        "fee",
        "payout",
        "reward",
        "interest",
        "premium",
        "price",
        "amount",
        "balance",
        "share",
        "supply",
        "collateral",
        "reserve",
        "liquidity",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

fn format_directions(summary: &BTreeSet<RoundingDirection>) -> String {
    summary
        .iter()
        .map(|direction| match direction {
            RoundingDirection::Down => "down",
            RoundingDirection::Nearest => "nearest",
            RoundingDirection::Up => "up",
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn safe_rounding_suggestion() -> String {
    "Define a single rounding policy for related financial flows, document who receives any remainder, and use explicit helpers such as `mul_div_floor` or `mul_div_ceil` consistently"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_mixed_rounding_inside_financial_function() {
        let source = r#"
            impl Contract {
                pub fn settle_fee(amount: i128, rate: i128, denominator: i128) -> i128 {
                    let maker = amount.mul_div_floor(rate, denominator);
                    let taker = amount.mul_div_ceil(rate, denominator);
                    maker + taker
                }
            }
        "#;

        let findings = RoundingDirectionRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].message.contains("down, up"));
    }

    #[test]
    fn flags_cross_function_related_rounding_mismatch() {
        let source = r#"
            impl Contract {
                pub fn deposit(amount: i128, rate: i128, denominator: i128) -> i128 {
                    amount.mul_div_ceil(rate, denominator)
                }

                pub fn withdraw(shares: i128, rate: i128, denominator: i128) -> i128 {
                    shares.mul_div_floor(rate, denominator)
                }
            }
        "#;

        let findings = RoundingDirectionRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("deposit"));
        assert!(findings[0].message.contains("withdraw"));
    }

    #[test]
    fn allows_consistent_rounding_policy() {
        let source = r#"
            impl Contract {
                pub fn deposit(amount: i128, rate: i128, denominator: i128) -> i128 {
                    amount.mul_div_floor(rate, denominator)
                }

                pub fn withdraw(shares: i128, rate: i128, denominator: i128) -> i128 {
                    shares.mul_div_floor(rate, denominator)
                }
            }
        "#;

        let findings = RoundingDirectionRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_unrelated_rounding() {
        let source = r#"
            fn layout(width: i128, denominator: i128) -> i128 {
                width.div_ceil(denominator)
            }
        "#;

        let findings = RoundingDirectionRule::new().check(source);

        assert!(findings.is_empty());
    }
}
