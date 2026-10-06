use crate::rules::{Rule, RuleViolation, Severity};
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::visit::Visit;

const FINDING_CODE: &str = "SANCT_NONCE_MONOTONICITY";

/// Detects nonce validation that accepts reused values or arbitrary gaps.
///
/// The detector is deliberately narrow: it inspects functions that accept a
/// nonce-named argument and then compares it with another nonce-shaped value.
/// Equality/inequality against an exact `stored_nonce + 1` expression is
/// treated as strict next-nonce handling. Direct equality or ordered
/// comparisons (`<`, `<=`, `>`, `>=`) are reported because they permit reuse
/// or skipping depending on which branch accepts the request. Storing a
/// caller-provided nonce without any strict next-nonce comparison is also
/// reported.
pub struct NonceMonotonicityRule;

impl NonceMonotonicityRule {
    pub fn new() -> Self {
        Self
    }

    fn check_fn(&self, sig: &syn::Signature, block: &syn::Block) -> Vec<RuleViolation> {
        if !signature_has_nonce_input(sig) {
            return Vec::new();
        }

        let mut visitor = NonceFlowVisitor::default();
        visitor.visit_block(block);

        let issue = visitor
            .weak_comparison
            .or_else(|| {
                if visitor.strict_next_seen {
                    None
                } else {
                    visitor.unvalidated_store
                }
            })
            .map(|candidate| match candidate.kind {
                CandidateKind::WeakComparison(op) => (
                    candidate.span,
                    format!(
                        "nonce comparison uses `{op}` instead of an exact next-nonce check"
                    ),
                    format!(
                        "Require the supplied nonce to equal the stored nonce plus one, for example \
                         `provided_nonce == stored_nonce + 1`, before updating storage."
                    ),
                ),
                CandidateKind::UnvalidatedStore => (
                    candidate.span,
                    "caller-provided nonce is stored without an exact next-nonce check".to_string(),
                    "Compare the supplied nonce with `stored_nonce + 1` and reject every other \
                     value before persisting it."
                        .to_string(),
                ),
            });

        let Some((span, reason, suggestion)) = issue else {
            return Vec::new();
        };

        vec![RuleViolation::new(
            FINDING_CODE,
            Severity::Warning,
            format!(
                "{FINDING_CODE}: `{}` has non-monotonic or skippable nonce handling: {reason}",
                sig.ident
            ),
            format!("{}:{}", sig.ident, span.start().line),
        )
        .with_suggestion(suggestion)]
    }
}

impl Default for NonceMonotonicityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for NonceMonotonicityRule {
    fn name(&self) -> &str {
        "nonce_monotonicity"
    }

    fn description(&self) -> &str {
        "Detects nonce checks that permit reuse or gaps instead of requiring exactly stored_nonce + 1."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };

        let mut visitor = FunctionVisitor {
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

struct FunctionVisitor<'a> {
    rule: &'a NonceMonotonicityRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FunctionVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations
            .extend(self.rule.check_fn(&node.sig, &node.block));
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations
            .extend(self.rule.check_fn(&node.sig, &node.block));
        syn::visit::visit_impl_item_fn(self, node);
    }
}

#[derive(Default)]
struct NonceFlowVisitor {
    strict_next_seen: bool,
    weak_comparison: Option<Candidate>,
    unvalidated_store: Option<Candidate>,
}

impl<'ast> Visit<'ast> for NonceFlowVisitor {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if is_nonce_comparison(node) {
            if is_strict_next_comparison(node) {
                self.strict_next_seen = true;
            } else if self.weak_comparison.is_none() {
                if let Some(op) = comparison_operator(&node.op) {
                    self.weak_comparison = Some(Candidate {
                        span: node.span(),
                        kind: CandidateKind::WeakComparison(op),
                    });
                }
            }
        }

        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "set"
            && self.unvalidated_store.is_none()
            && node.args.last().map(expr_contains_nonce).unwrap_or(false)
        {
            self.unvalidated_store = Some(Candidate {
                span: node.span(),
                kind: CandidateKind::UnvalidatedStore,
            });
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

struct Candidate {
    span: proc_macro2::Span,
    kind: CandidateKind,
}

enum CandidateKind {
    WeakComparison(&'static str),
    UnvalidatedStore,
}

fn signature_has_nonce_input(sig: &syn::Signature) -> bool {
    sig.inputs.iter().any(|input| {
        input
            .to_token_stream()
            .to_string()
            .to_ascii_lowercase()
            .contains("nonce")
    })
}

fn is_nonce_comparison(binary: &syn::ExprBinary) -> bool {
    comparison_operator(&binary.op).is_some()
        && expr_contains_nonce(&binary.left)
        && expr_contains_nonce(&binary.right)
}

fn is_strict_next_comparison(binary: &syn::ExprBinary) -> bool {
    if !matches!(&binary.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
        return false;
    }

    (is_exact_nonce_increment(&binary.left) && expr_contains_nonce(&binary.right))
        || (is_exact_nonce_increment(&binary.right) && expr_contains_nonce(&binary.left))
}

fn is_exact_nonce_increment(expr: &syn::Expr) -> bool {
    let expr = unwrap_expr(expr);
    let syn::Expr::Binary(binary) = expr else {
        return false;
    };
    if !matches!(&binary.op, syn::BinOp::Add(_)) {
        return false;
    }

    (expr_contains_nonce(&binary.left) && is_integer_one(&binary.right))
        || (is_integer_one(&binary.left) && expr_contains_nonce(&binary.right))
}

fn is_integer_one(expr: &syn::Expr) -> bool {
    matches!(
        unwrap_expr(expr),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(value),
            ..
        }) if value.base10_parse::<u128>().ok() == Some(1)
    )
}

fn unwrap_expr(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(paren) => unwrap_expr(&paren.expr),
        syn::Expr::Group(group) => unwrap_expr(&group.expr),
        syn::Expr::Reference(reference) => unwrap_expr(&reference.expr),
        other => other,
    }
}

fn expr_contains_nonce(expr: &syn::Expr) -> bool {
    let mut visitor = NonceIdentifierVisitor { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

struct NonceIdentifierVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for NonceIdentifierVisitor {
    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        if ident.to_string().to_ascii_lowercase().contains("nonce") {
            self.found = true;
        }
    }

    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        if literal.value().to_ascii_lowercase().contains("nonce") {
            self.found = true;
        }
    }
}

fn comparison_operator(op: &syn::BinOp) -> Option<&'static str> {
    match op {
        syn::BinOp::Eq(_) => Some("=="),
        syn::BinOp::Ne(_) => Some("!="),
        syn::BinOp::Lt(_) => Some("<"),
        syn::BinOp::Le(_) => Some("<="),
        syn::BinOp::Gt(_) => Some(">"),
        syn::BinOp::Ge(_) => Some(">="),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_gap_permitting_ordered_check() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(env: Env, provided_nonce: u64) {
                let stored_nonce: u64 = env.storage().instance().get(&"nonce").unwrap_or(0);
                if provided_nonce <= stored_nonce {
                    panic!("replay");
                }
                env.storage().instance().set(&"nonce", &provided_nonce);
            }
            "#,
        );

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("`<=`"));
    }

    #[test]
    fn flags_direct_nonce_equality() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(env: Env, provided_nonce: u64) {
                let stored_nonce: u64 = env.storage().instance().get(&"nonce").unwrap_or(0);
                if provided_nonce == stored_nonce {
                    execute_once();
                }
            }
            "#,
        );

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("`==`"));
    }

    #[test]
    fn ignores_strict_next_nonce_check() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(env: Env, provided_nonce: u64) {
                let stored_nonce: u64 = env.storage().instance().get(&"nonce").unwrap_or(0);
                if provided_nonce != stored_nonce + 1 {
                    panic!("invalid nonce");
                }
                env.storage().instance().set(&"nonce", &provided_nonce);
            }
            "#,
        );

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn recognizes_reversed_strict_increment() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(env: Env, supplied_nonce: u64) {
                let current_nonce: u64 = env.storage().instance().get(&"nonce").unwrap_or(0);
                if current_nonce + 1 == supplied_nonce {
                    apply();
                }
            }
            "#,
        );

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn flags_weak_comparison_even_when_another_nonce_check_is_strict() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(
                env: Env,
                auth_nonce: u64,
                transfer_nonce: u64,
            ) {
                let stored_auth_nonce: u64 =
                    env.storage().instance().get(&"auth_nonce").unwrap_or(0);
                if auth_nonce != stored_auth_nonce + 1 {
                    panic!("invalid auth nonce");
                }

                let stored_transfer_nonce: u64 =
                    env.storage().instance().get(&"transfer_nonce").unwrap_or(0);
                if transfer_nonce <= stored_transfer_nonce {
                    panic!("replayed transfer nonce");
                }
            }
            "#,
        );

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("`<=`"));
    }

    #[test]
    fn flags_nonce_store_without_validation() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(env: Env, provided_nonce: u64) {
                env.storage().instance().set(&"nonce", &provided_nonce);
            }
            "#,
        );

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("stored without"));
    }

    #[test]
    fn ignores_unrelated_ordered_comparison() {
        let findings = NonceMonotonicityRule::new().check(
            r#"
            pub fn execute(env: Env, amount: u64) {
                if amount > 0 {
                    apply();
                }
            }
            "#,
        );

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn fixture_snapshot() {
        let findings = NonceMonotonicityRule::new().check(include_str!(
            "../../tests/fixtures/detectors/nonce_monotonicity.rs"
        ));

        insta::assert_yaml_snapshot!(findings, @r###"
        - rule_name: SANCT_NONCE_MONOTONICITY
          severity: Warning
          message: "SANCT_NONCE_MONOTONICITY: `execute` has non-monotonic or skippable nonce handling: nonce comparison uses `<=` instead of an exact next-nonce check"
          location: "execute:16"
          suggestion: "Require the supplied nonce to equal the stored nonce plus one, for example `provided_nonce == stored_nonce + 1`, before updating storage."
        "###);
    }
}
