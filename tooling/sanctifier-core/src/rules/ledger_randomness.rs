use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_LEDGER_RANDOMNESS";

/// Detects ledger sequence/timestamp values used as a pseudo-random source.
pub struct LedgerRandomnessRule;

impl LedgerRandomnessRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LedgerRandomnessRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for LedgerRandomnessRule {
    fn name(&self) -> &str {
        "ledger_randomness"
    }

    fn description(&self) -> &str {
        "Detects ledger sequence or timestamp values used as predictable randomness"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = LedgerRandomnessVisitor {
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

struct LedgerRandomnessVisitor {
    current_fn: Option<String>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for LedgerRandomnessVisitor {
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
        if matches!(node.op, syn::BinOp::Rem(_))
            && (expr_contains_ledger_time(&node.left) || expr_contains_ledger_time(&node.right))
        {
            self.push_violation(node.span().start().line, "ledger value is reduced with `%`");
        }

        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref() {
            if let Some(segment) = path.path.segments.last() {
                let function_name = segment.ident.to_string();
                if is_randomness_sink(&function_name)
                    && node.args.iter().any(expr_contains_ledger_time)
                {
                    self.push_violation(
                        node.span().start().line,
                        "ledger value flows into a randomness-like helper",
                    );
                }
            }
        }

        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if is_randomness_sink(&method)
            && (expr_contains_ledger_time(&node.receiver)
                || node.args.iter().any(expr_contains_ledger_time))
        {
            self.push_violation(
                node.span().start().line,
                "ledger value flows into a randomness-like method",
            );
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

impl LedgerRandomnessVisitor {
    fn push_violation(&mut self, line: usize, reason: &str) {
        let fn_name = self.current_fn.as_deref().unwrap_or("<module>");
        if !is_randomness_context(fn_name) {
            return;
        }

        self.violations.push(
            RuleViolation::new(
                FINDING_CODE,
                Severity::Warning,
                format!(
                    "{FINDING_CODE}: {reason}; ledger sequence/timestamp is predictable and manipulable as randomness"
                ),
                format!("{}:{}", fn_name, line),
            )
            .with_suggestion(
                "Use commit-reveal, a VRF/oracle, or a user-independent entropy source instead of `env.ledger().sequence()` or `timestamp()`"
                    .to_string(),
            ),
        );
    }
}

fn expr_contains_ledger_time(expr: &syn::Expr) -> bool {
    let mut visitor = LedgerTimeVisitor { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

struct LedgerTimeVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for LedgerTimeVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if matches!(method.as_str(), "sequence" | "timestamp") && receiver_mentions_ledger(node) {
            self.found = true;
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

fn receiver_mentions_ledger(node: &syn::ExprMethodCall) -> bool {
    let tokens = quote::quote!(#node.receiver).to_string();
    tokens.contains("ledger")
}

fn is_randomness_context(fn_name: &str) -> bool {
    let lower = fn_name.to_ascii_lowercase();
    [
        "random", "rand", "rng", "seed", "nonce", "shuffle", "draw", "lottery", "raffle", "winner",
        "pick", "select", "sample", "roll",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

fn is_randomness_sink(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "hash", "sha", "keccak", "random", "rand", "rng", "seed", "shuffle", "pick", "select",
        "index",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_ledger_sequence_modulo_in_random_context() {
        let source = r#"
            impl Contract {
                pub fn pick_winner(env: Env, participant_count: u32) -> u32 {
                    env.ledger().sequence() % participant_count
                }
            }
        "#;

        let findings = LedgerRandomnessRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("commit-reveal"));
    }

    #[test]
    fn flags_ledger_timestamp_seeded_hash() {
        let source = r#"
            impl Contract {
                pub fn random_seed(env: Env) -> BytesN<32> {
                    hash(env.ledger().timestamp())
                }
            }
        "#;

        let findings = LedgerRandomnessRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("randomness-like helper"));
    }

    #[test]
    fn ignores_plain_expiry_timestamp() {
        let source = r#"
            impl Contract {
                pub fn expires_at(env: Env, ttl: u64) -> u64 {
                    env.ledger().timestamp() + ttl
                }
            }
        "#;

        let findings = LedgerRandomnessRule::new().check(source);

        assert!(findings.is_empty());
    }
}
