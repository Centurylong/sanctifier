use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects ledgers-vs-seconds confusion: a ledger **sequence number**
/// (`env.ledger().sequence()`) combined in the same expression with an integer
/// literal that clearly denotes a duration in **seconds**.
///
/// A Soroban ledger sequence is a monotonic block counter that advances roughly
/// once every ~5 seconds, so `ledger().sequence() + 86_400` does not mean "one
/// day from now" — it means "86,400 ledgers", i.e. days of real time. Time
/// windows expressed in seconds belong with `env.ledger().timestamp()` instead.
/// The rule fires only when a `.sequence()` call meets a seconds-magnitude
/// literal (>= 60) across an arithmetic or comparison operator, keeping the
/// signal high and leaving timestamp-based math alone.
pub struct LedgerSecondsRule;

/// Smallest literal (in seconds) we treat as an unmistakable duration. One
/// minute of ledgers would be ~12 blocks, so a literal this large next to a
/// sequence number is almost certainly a unit mix-up.
const SECONDS_THRESHOLD: u64 = 60;

impl LedgerSecondsRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LedgerSecondsRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for LedgerSecondsRule {
    fn name(&self) -> &str {
        "ledger_seconds"
    }

    fn description(&self) -> &str {
        "Detects ledger sequence numbers mixed with seconds-magnitude literals (ledgers-vs-seconds confusion)"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = SeqVisitor {
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

struct SeqVisitor {
    fn_name: String,
    seen: HashSet<usize>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for SeqVisitor {
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

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let arithmetic_or_cmp = matches!(
            node.op,
            syn::BinOp::Add(_)
                | syn::BinOp::Sub(_)
                | syn::BinOp::Lt(_)
                | syn::BinOp::Le(_)
                | syn::BinOp::Gt(_)
                | syn::BinOp::Ge(_)
        );
        if arithmetic_or_cmp {
            let (l, r) = (&*node.left, &*node.right);
            let mixes = (contains_sequence_call(l) && seconds_literal(r).is_some())
                || (contains_sequence_call(r) && seconds_literal(l).is_some());
            if mixes {
                let secs = seconds_literal(l)
                    .or_else(|| seconds_literal(r))
                    .unwrap_or(0);
                let line = node.span().start().line;
                if self.seen.insert(line) {
                    self.violations.push(
                        RuleViolation::new(
                            "ledger_seconds",
                            Severity::Warning,
                            format!(
                                "Ledger sequence number mixed with a seconds-magnitude literal `{secs}`; \
                                 sequence numbers count blocks (~5s each), not seconds"
                            ),
                            format!("{}:{}", self.fn_name, line),
                        )
                        .with_suggestion(
                            "Use `env.ledger().timestamp()` for time windows measured in seconds, \
                             or convert the duration to a ledger count before adding it to `sequence()`"
                                .to_string(),
                        ),
                    );
                }
            }
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

/// Whether `expr` contains a `.sequence()` method call anywhere within it.
fn contains_sequence_call(expr: &syn::Expr) -> bool {
    struct Finder {
        found: bool,
    }
    impl<'ast> Visit<'ast> for Finder {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if node.method == "sequence" && node.args.is_empty() {
                self.found = true;
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }
    let mut f = Finder { found: false };
    f.visit_expr(expr);
    f.found
}

/// If `expr` is an integer literal that is a plausible seconds duration
/// (>= SECONDS_THRESHOLD), return its value.
fn seconds_literal(expr: &syn::Expr) -> Option<u64> {
    if let syn::Expr::Lit(lit) = expr {
        if let syn::Lit::Int(int) = &lit.lit {
            if let Ok(v) = int.base10_parse::<u64>() {
                if v >= SECONDS_THRESHOLD {
                    return Some(v);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_sequence_plus_seconds_literal() {
        let rule = LedgerSecondsRule::new();
        let source = r#"
            impl Contract {
                pub fn expiry(env: Env) -> u32 {
                    env.ledger().sequence() + 86400
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("sequence"));
    }

    #[test]
    fn flags_comparison() {
        let rule = LedgerSecondsRule::new();
        let source = r#"
            impl Contract {
                pub fn expired(env: Env, deadline: u32) -> bool {
                    deadline < env.ledger().sequence() - 3600
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn ignores_timestamp_math() {
        let rule = LedgerSecondsRule::new();
        let source = r#"
            impl Contract {
                pub fn expiry(env: Env) -> u64 {
                    env.ledger().timestamp() + 86400
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_small_ledger_deltas() {
        let rule = LedgerSecondsRule::new();
        let source = r#"
            impl Contract {
                pub fn next(env: Env) -> u32 {
                    env.ledger().sequence() + 10
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }
}
