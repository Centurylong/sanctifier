use crate::finding_codes::BALANCE_EQUALITY;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Flags `==` / `!=` comparisons that gate a spend on a balance being *exactly*
/// equal to an amount, where an inequality (`>=` / `<=`) was almost certainly
/// intended.
///
/// Gating on `balance == amount` (instead of `balance >= amount`) causes
/// spurious failures for every balance that isn't a pixel-perfect match, and in
/// some flows enables edge exploits. This is an advisory detector: it only fires
/// when one side looks like a *balance/holdings* quantity and the other like an
/// *amount/spend* quantity, so ordinary emptiness checks (`amount == 0`) and
/// unrelated equality are left alone.
pub struct BalanceEqualityRule;

impl BalanceEqualityRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BalanceEqualityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for BalanceEqualityRule {
    fn name(&self) -> &str {
        "balance_equality"
    }

    fn description(&self) -> &str {
        "Detects `==`/`!=` gating a balance against an amount where `>=`/`<=` was intended"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = BalanceEqualityVisitor {
            violations: Vec::new(),
            current_fn: None,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Quantity {
    Balance,
    Amount,
    Other,
}

/// Substrings that mark an operand as a stored *balance/holdings* quantity.
const BALANCE_HINTS: &[&str] = &[
    "balance",
    "reserve",
    "supply",
    "funds",
    "liquidity",
    "collateral",
    "vault",
    "holdings",
    "escrow",
    "deposited",
];

/// Substrings that mark an operand as a requested *amount/spend* quantity.
const AMOUNT_HINTS: &[&str] = &[
    "amount", "amt", "withdraw", "spend", "payment", "payout", "debit", "required", "owed",
    "price", "cost", "quantity", "qty",
];

fn classify(name: &str) -> Quantity {
    let lower = name.to_lowercase();
    // Balance hints win first: `get_balance` contains neither amount hint.
    if BALANCE_HINTS.iter().any(|hint| lower.contains(hint)) {
        Quantity::Balance
    } else if AMOUNT_HINTS.iter().any(|hint| lower.contains(hint)) {
        Quantity::Amount
    } else {
        Quantity::Other
    }
}

/// Extract a comparable name from an operand: a path/ident, a field access
/// (`self.balance` -> `balance`), a method call (`x.balance()` -> `balance`), or
/// a free call (`get_balance(..)` -> `get_balance`). Literals yield `None` so
/// `balance == 0` is never flagged.
fn operand_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        syn::Expr::Field(field) => match &field.member {
            syn::Member::Named(ident) => Some(ident.to_string()),
            syn::Member::Unnamed(_) => None,
        },
        syn::Expr::MethodCall(call) => Some(call.method.to_string()),
        syn::Expr::Call(call) => operand_name(&call.func),
        syn::Expr::Reference(reference) => operand_name(&reference.expr),
        syn::Expr::Paren(paren) => operand_name(&paren.expr),
        syn::Expr::Group(group) => operand_name(&group.expr),
        _ => None,
    }
}

struct BalanceEqualityVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
}

impl BalanceEqualityVisitor {
    fn with_fn<F: FnOnce(&mut Self)>(&mut self, name: String, body: F) {
        let previous = self.current_fn.replace(name);
        body(self);
        self.current_fn = previous;
    }

    fn location(&self, line: usize) -> String {
        match &self.current_fn {
            Some(name) => format!("{name}:{line}"),
            None => format!("line {line}"),
        }
    }
}

impl<'ast> Visit<'ast> for BalanceEqualityVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let name = node.sig.ident.to_string();
        self.with_fn(name, |v| syn::visit::visit_item_fn(v, node));
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let name = node.sig.ident.to_string();
        self.with_fn(name, |v| syn::visit::visit_impl_item_fn(v, node));
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let is_equality = matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_));
        if is_equality {
            if let (Some(left), Some(right)) = (operand_name(&node.left), operand_name(&node.right))
            {
                let pair = (classify(&left), classify(&right));
                let flagged = matches!(
                    pair,
                    (Quantity::Balance, Quantity::Amount) | (Quantity::Amount, Quantity::Balance)
                );
                if flagged {
                    let op = if matches!(node.op, syn::BinOp::Eq(_)) {
                        "=="
                    } else {
                        "!="
                    };
                    let line = node.op.span().start().line;
                    self.violations.push(
                        RuleViolation::new(
                            BALANCE_EQUALITY,
                            Severity::Info,
                            format!(
                                "{BALANCE_EQUALITY}: gating `{left} {op} {right}` compares a balance and an amount for exact (in)equality; `>=`/`<=` was likely intended",
                            ),
                            self.location(line),
                        )
                        .with_suggestion(format!(
                            "Use `{left} >= {right}` (or `<=`) instead of `{op}` when gating a spend on a balance; exact equality can lock funds or enable edge exploits",
                        )),
                    );
                }
            }
        }

        syn::visit::visit_expr_binary(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_balance_equals_amount_gate() {
        let source = r#"
            impl Vault {
                pub fn withdraw(env: Env, from: Address, amount: i128) {
                    let balance = get_balance(&env, from.clone());
                    if balance == amount {
                        do_withdraw(&env, from, amount);
                    }
                }
            }
        "#;
        let findings = BalanceEqualityRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, BALANCE_EQUALITY);
        assert!(findings[0].location.contains("withdraw"));
    }

    #[test]
    fn flags_call_and_ne_forms() {
        let source = r#"
            impl Vault {
                pub fn pay(env: Env, to: Address, amount: i128) {
                    if get_balance(&env, to.clone()) != amount {
                        panic!("no");
                    }
                }
            }
        "#;
        let findings = BalanceEqualityRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("!="));
    }

    #[test]
    fn ignores_zero_and_inequality_checks() {
        let source = r#"
            impl Vault {
                pub fn withdraw(env: Env, from: Address, amount: i128) {
                    if amount == 0 {
                        panic!("zero");
                    }
                    let balance = get_balance(&env, from.clone());
                    if balance >= amount {
                        do_withdraw(&env, from, amount);
                    }
                }
            }
        "#;
        let findings = BalanceEqualityRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_unrelated_equality() {
        let source = r#"
            impl Vault {
                pub fn check(env: Env, a: u32, b: u32) -> bool {
                    a == b
                }
            }
        "#;
        let findings = BalanceEqualityRule::new().check(source);
        assert!(findings.is_empty());
    }
}
