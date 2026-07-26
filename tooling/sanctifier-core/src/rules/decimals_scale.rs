use crate::rules::{Rule, RuleViolation, Severity};
use quote::ToTokens;
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Expr, File};

const FINDING_CODE: &str = "SANCT_DECIMALS";

/// Detects arithmetic that mixes raw token amounts from different sources or raw and scaled
/// amounts without an obvious normalization step.
pub struct DecimalsScaleRule;

impl DecimalsScaleRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DecimalsScaleRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DecimalsScaleRule {
    fn name(&self) -> &str {
        FINDING_CODE
    }

    fn description(&self) -> &str {
        "Detects token arithmetic that mixes raw and scaled amounts without decimals/scale validation"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = DecimalsScaleVisitor::default();
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct DecimalsScaleVisitor {
    aliases: HashMap<String, AmountInfo>,
    violations: Vec<RuleViolation>,
    fn_stack: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AmountInfo {
    source: String,
    scaled: bool,
}

impl<'ast> Visit<'ast> for DecimalsScaleVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.fn_stack.push(node.sig.ident.to_string());
        self.visit_block(&node.block);
        self.fn_stack.pop();
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.fn_stack.push(node.sig.ident.to_string());
        self.visit_block(&node.block);
        self.fn_stack.pop();
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(init) = &node.init {
            self.check_expr(&init.expr);
            if let syn::Pat::Ident(pat) = &node.pat {
                if let Some(info) = self.amount_info(&init.expr) {
                    self.aliases.insert(pat.ident.to_string(), info);
                }
            }
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr(&mut self, node: &'ast Expr) {
        self.check_expr(node);
        syn::visit::visit_expr(self, node);
    }
}

impl DecimalsScaleVisitor {
    fn check_expr(&mut self, expr: &Expr) {
        let Expr::Binary(binary) = expr else {
            return;
        };
        if !matches!(
            binary.op,
            syn::BinOp::Add(_)
                | syn::BinOp::Sub(_)
                | syn::BinOp::Eq(_)
                | syn::BinOp::Ne(_)
                | syn::BinOp::Lt(_)
                | syn::BinOp::Le(_)
                | syn::BinOp::Gt(_)
                | syn::BinOp::Ge(_)
        ) {
            return;
        }
        if expr_has_scale_validation(expr) {
            return;
        }
        let (Some(left), Some(right)) = (
            self.amount_info(&binary.left),
            self.amount_info(&binary.right),
        ) else {
            return;
        };
        if left.source == right.source && left.scaled == right.scaled {
            return;
        }
        let line = binary.op.span().start().line;
        let fn_name = self
            .fn_stack
            .last()
            .map(String::as_str)
            .unwrap_or("<module>");
        self.violations.push(
            RuleViolation::new(
                FINDING_CODE,
                Severity::Warning,
                format!(
                    "{FINDING_CODE}: token amount arithmetic compares or combines `{}` ({}) with `{}` ({}) without explicit decimals/scale normalization",
                    expr_to_string(&binary.left), describe(&left), expr_to_string(&binary.right), describe(&right)
                ),
                format!("{}:{}", fn_name, line),
            )
            .with_suggestion(
                "Normalize token amounts to a documented common scale and validate token decimals before adding, subtracting, or comparing them.".to_string(),
            ),
        );
    }

    fn amount_info(&self, expr: &Expr) -> Option<AmountInfo> {
        if expr_has_scale_validation(expr) {
            return None;
        }
        match expr {
            Expr::Path(path) => {
                let name = path.path.segments.last()?.ident.to_string();
                self.aliases
                    .get(&name)
                    .cloned()
                    .or_else(|| info_from_name(&name))
            }
            Expr::Field(field) => info_from_name(&expr_to_string(&Expr::Field(field.clone()))),
            Expr::MethodCall(call) => {
                let method = call.method.to_string();
                let mut info =
                    info_from_name(&method).or_else(|| self.amount_info(&call.receiver))?;
                info.scaled |= is_scaled_name(&method);
                Some(info)
            }
            Expr::Call(call) => call.args.iter().find_map(|arg| self.amount_info(arg)),
            Expr::Paren(paren) => self.amount_info(&paren.expr),
            _ => None,
        }
    }
}

fn info_from_name(name: &str) -> Option<AmountInfo> {
    let lower = name.to_lowercase();
    if !(lower.contains("amount") || lower.contains("balance") || lower.contains("supply")) {
        return None;
    }
    let source = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .find(|part| {
            !part.is_empty()
                && !matches!(
                    *part,
                    "raw" | "scaled" | "amount" | "balance" | "supply" | "token"
                )
        })
        .unwrap_or("token")
        .to_string();
    Some(AmountInfo {
        source,
        scaled: is_scaled_name(&lower),
    })
}

fn is_scaled_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("scaled") || lower.contains("normalized") || lower.contains("decimal")
}

fn expr_has_scale_validation(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => validation_name(&expr_to_string(&call.func)),
        Expr::MethodCall(call) => validation_name(&call.method.to_string()),
        Expr::Binary(binary) => {
            expr_has_scale_validation(&binary.left) || expr_has_scale_validation(&binary.right)
        }
        Expr::Paren(paren) => expr_has_scale_validation(&paren.expr),
        _ => false,
    }
}

fn validation_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "decimal",
        "scale",
        "normalize",
        "normalise",
        "to_raw",
        "from_raw",
        "pow10",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn expr_to_string(expr: &Expr) -> String {
    expr.to_token_stream().to_string().replace(' ', "")
}

fn describe(info: &AmountInfo) -> String {
    format!(
        "{} {}",
        info.source,
        if info.scaled { "scaled" } else { "raw" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_different_token_sources_added_directly() {
        let rule = DecimalsScaleRule::new();
        let source = r#"
            pub fn total(token_a_amount: i128, token_b_amount: i128) -> i128 {
                token_a_amount + token_b_amount
            }
        "#;
        let findings = rule.check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, "SANCT_DECIMALS");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].message.contains("SANCT_DECIMALS"));
    }

    #[test]
    fn flags_raw_and_scaled_amount_comparison() {
        let rule = DecimalsScaleRule::new();
        let source = r#"
            pub fn enough(usdc_amount: i128, usdc_scaled_amount: i128) -> bool {
                usdc_amount >= usdc_scaled_amount
            }
        "#;
        let findings = rule.check(source);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn ignores_documented_normalization_step() {
        let rule = DecimalsScaleRule::new();
        let source = r#"
            pub fn total(token_a_amount: i128, token_b_amount: i128) -> i128 {
                let token_b_normalized = normalize_amount(token_b_amount, token_b_decimals(), token_a_decimals());
                token_a_amount + token_b_normalized
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn ignores_same_source_same_scale() {
        let rule = DecimalsScaleRule::new();
        let source = r#"
            pub fn total(usdc_amount: i128, usdc_fee_amount: i128) -> i128 {
                usdc_amount - usdc_fee_amount
            }
        "#;
        assert!(rule.check(source).is_empty());
    }
}
