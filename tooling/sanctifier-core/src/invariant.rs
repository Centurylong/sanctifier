use serde::Serialize;
use syn::{spanned::Spanned, visit::Visit, Attribute, File, ItemImpl};

/// Walk a parsed `File` and collect every `#[sanctify::invariant(...)]` found
/// on `impl` blocks.
pub fn scan_invariant_attrs(source: &str, file_label: &str) -> Vec<InvariantDecl> {
    scan_attrs_named(source, file_label, "invariant")
}

/// Walk a parsed `File` and collect every `#[sanctify::assume(...)]` (or
/// `#[assume(...)]`) found on `impl` blocks.
///
/// An assumption bounds the state a paired `#[sanctify::invariant(...)]` is
/// checked against — see [`SmtInvariantVerifier::verify_one_with_assumptions`].
/// It is not itself something to prove.
pub fn scan_assume_attrs(source: &str, file_label: &str) -> Vec<InvariantDecl> {
    scan_attrs_named(source, file_label, "assume")
}

/// Walk a parsed `File` and collect every `#[sanctify::<name>(...)]` (or
/// `#[<name>(...)]`) attribute found on `impl` blocks.
fn scan_attrs_named(source: &str, file_label: &str, name: &str) -> Vec<InvariantDecl> {
    let ast: File = match syn::parse_str(source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut visitor = InvariantVisitor {
        decls: Vec::new(),
        file_label: file_label.to_string(),
        attr_name: name.to_string(),
    };
    visitor.visit_file(&ast);
    visitor.decls
}

struct InvariantVisitor {
    decls: Vec<InvariantDecl>,
    file_label: String,
    attr_name: String,
}

impl<'ast> Visit<'ast> for InvariantVisitor {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        for attr in &node.attrs {
            if let Some(expr_str) = extract_attr_expr(attr, &self.attr_name) {
                let contract_name = impl_self_name(node);
                let line = node.span().start().line;
                self.decls.push(InvariantDecl {
                    contract_name,
                    expr_str,
                    location: format!("{}:{}", self.file_label, line),
                });
            }
        }
        syn::visit::visit_item_impl(self, node);
    }
}

/// Return the expression string if `attr` is `#[sanctify::<name>(...)]` or
/// `#[<name>(...)]`, otherwise `None`.
fn extract_attr_expr(attr: &Attribute, name: &str) -> Option<String> {
    let path = attr.path();
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    let matches_name = match segments.as_slice() {
        [n] => n == name,
        [ns, n] => ns == "sanctify" && n == name,
        _ => false,
    };

    if !matches_name {
        return None;
    }

    // attr.parse_args::<syn::Expr>() gives us the inner tokens; convert back to string.
    match attr.parse_args::<syn::Expr>() {
        Ok(expr) => Some(quote::quote!(#expr).to_string()),
        Err(_) => {
            // Fall back to raw token string if parsing as Expr fails.
            if let syn::Meta::List(ml) = &attr.meta {
                Some(ml.tokens.to_string())
            } else {
                None
            }
        }
    }
}

/// Best-effort name of the impl's self-type.
fn impl_self_name(node: &ItemImpl) -> String {
    quote::quote!(#node.self_ty)
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A `#[sanctify::invariant(EXPR)]` declaration extracted from a source file.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InvariantDecl {
    /// Name of the `impl` self-type the attribute was placed on.
    pub contract_name: String,
    /// The raw invariant expression as it appears in source.
    pub expr_str: String,
    /// Human-readable location string (`file:line`).
    pub location: String,
}

/// The outcome of attempting to verify one `InvariantDecl`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InvariantVerifyResult {
    /// The SMT solver proved the invariant holds for all inputs.
    Proven,
    /// The SMT solver found a counterexample (the invariant can be violated).
    Refuted { counterexample: String },
    /// The solver timed out or returned unknown.
    Unknown,
    /// The invariant expression is not in a form the SMT backend can check
    /// (e.g. it calls user functions). Dispatch to Kani instead.
    Unsupported,
}

// ── SMT-backed verifier ───────────────────────────────────────────────────────

/// Attempts to verify an `InvariantDecl` using the Z3 SMT backend.
///
/// Only a subset of expressions can be dispatched to Z3: simple arithmetic
/// equalities of the form `a == b` where both sides are integer literals or
/// unconstrained symbolic integers. Everything else returns `Unsupported` so
/// the caller can redirect to Kani.
#[cfg(feature = "smt")]
pub struct SmtInvariantVerifier;

#[cfg(feature = "smt")]
impl Default for SmtInvariantVerifier {
    fn default() -> Self {
        SmtInvariantVerifier
    }
}

#[cfg(feature = "smt")]
impl SmtInvariantVerifier {
    pub fn new() -> Self {
        SmtInvariantVerifier
    }

    /// Try to verify a single invariant declaration.
    pub fn verify_one(&self, decl: &InvariantDecl) -> InvariantVerifyResult {
        use z3::ast::{Ast, Int};
        use z3::{Config, Context, SatResult, Solver};

        // Parse `lhs == rhs` where both sides are decimal integer literals.
        if let Some((lhs, rhs)) = parse_integer_equality(&decl.expr_str) {
            let cfg = Config::new();
            let ctx = Context::new(&cfg);
            let solver = Solver::new(&ctx);

            let l = Int::from_i64(&ctx, lhs);
            let r = Int::from_i64(&ctx, rhs);

            // Assert the negation: if the solver can't find a model for !(l == r)
            // then l == r is always true (proven). Otherwise it's refuted.
            solver.assert(&l._eq(&r).not());

            return match solver.check() {
                SatResult::Unsat => InvariantVerifyResult::Proven,
                SatResult::Sat => InvariantVerifyResult::Refuted {
                    counterexample: format!("{} != {}", lhs, rhs),
                },
                SatResult::Unknown => InvariantVerifyResult::Unknown,
            };
        }

        // Parse `a == a` style tautologies with matching identifiers.
        if let Some(true) = parse_tautological_equality(&decl.expr_str) {
            return InvariantVerifyResult::Proven;
        }

        // Expression involves user-defined functions or complex terms — defer to Kani.
        InvariantVerifyResult::Unsupported
    }

    /// Like [`verify_one`](Self::verify_one), but additionally bounds the
    /// state using `#[sanctify::assume(...)]` declarations scoped to the
    /// same contract.
    ///
    /// This is how assume/assert annotations make an otherwise-undecidable
    /// property tractable: `verify_one` alone can only prove an invariant of
    /// the form `IDENT == LITERAL` when `IDENT` is itself a literal (or the
    /// two sides are token-identical). A bare identifier is unconstrained, so
    /// on its own the invariant is `Unsupported` and gets punted to Kani. An
    /// assumption `#[sanctify::assume(IDENT == LITERAL)]` on the same impl
    /// pins that identifier to a concrete value for the SMT pass, which is
    /// enough to decide simple equality invariants over it without a full
    /// symbolic-execution run.
    ///
    /// Assumptions that don't apply to any free identifier in `decl` are
    /// ignored. Contradictory assumptions on the same identifier make the
    /// assumed state unsatisfiable, so the invariant is vacuously `Proven` —
    /// the same convention formal-verification tools use for an infeasible
    /// precondition.
    pub fn verify_one_with_assumptions(
        &self,
        decl: &InvariantDecl,
        assumptions: &[InvariantDecl],
    ) -> InvariantVerifyResult {
        // The unconditional fast path (literal equality / tautology) never
        // needs assumptions and always takes priority.
        let unconditional = self.verify_one(decl);
        if unconditional != InvariantVerifyResult::Unsupported {
            return unconditional;
        }

        let Some((ident, target)) = parse_symbolic_equality(&decl.expr_str) else {
            return InvariantVerifyResult::Unsupported;
        };

        let relevant: Vec<(String, i64)> = assumptions
            .iter()
            .filter(|a| a.contract_name == decl.contract_name)
            .filter_map(|a| parse_symbolic_equality(&a.expr_str))
            .filter(|(name, _)| *name == ident)
            .collect();

        if relevant.is_empty() {
            return InvariantVerifyResult::Unsupported;
        }

        use z3::ast::{Ast, Int};
        use z3::{Config, Context, SatResult, Solver};

        let cfg = Config::new();
        let ctx = Context::new(&cfg);
        let solver = Solver::new(&ctx);

        let var = Int::new_const(&ctx, ident.as_str());
        for (_, value) in &relevant {
            solver.assert(&var._eq(&Int::from_i64(&ctx, *value)));
        }

        let target_const = Int::from_i64(&ctx, target);
        solver.assert(&var._eq(&target_const).not());

        match solver.check() {
            SatResult::Unsat => InvariantVerifyResult::Proven,
            SatResult::Sat => {
                let assumed = relevant
                    .iter()
                    .map(|(n, v)| format!("{n} == {v}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                InvariantVerifyResult::Refuted {
                    counterexample: format!(
                        "assuming {assumed}, {ident} != {target} is satisfiable"
                    ),
                }
            }
            SatResult::Unknown => InvariantVerifyResult::Unknown,
        }
    }

    /// Verify all declarations and return paired results.
    pub fn verify_all(
        &self,
        decls: &[InvariantDecl],
    ) -> Vec<(InvariantDecl, InvariantVerifyResult)> {
        decls
            .iter()
            .map(|d| (d.clone(), self.verify_one(d)))
            .collect()
    }

    /// Verify all declarations, bounding each with any `assumptions` scoped
    /// to the same contract (see [`verify_one_with_assumptions`](Self::verify_one_with_assumptions)).
    pub fn verify_all_with_assumptions(
        &self,
        decls: &[InvariantDecl],
        assumptions: &[InvariantDecl],
    ) -> Vec<(InvariantDecl, InvariantVerifyResult)> {
        decls
            .iter()
            .map(|d| (d.clone(), self.verify_one_with_assumptions(d, assumptions)))
            .collect()
    }
}

/// Parse `"N == M"` where N and M are i64 decimal literals.
#[cfg(feature = "smt")]
fn parse_integer_equality(expr: &str) -> Option<(i64, i64)> {
    let expr = expr.trim();
    let parts: Vec<&str> = expr.splitn(2, "==").collect();
    if parts.len() != 2 {
        return None;
    }
    let lhs = parts[0].trim().parse::<i64>().ok()?;
    let rhs = parts[1].trim().parse::<i64>().ok()?;
    Some((lhs, rhs))
}

/// Return `Some(true)` when the expression is of the form `x == x` (same
/// token on both sides), which is always a tautology.
#[cfg(feature = "smt")]
fn parse_tautological_equality(expr: &str) -> Option<bool> {
    let expr = expr.trim();
    let parts: Vec<&str> = expr.splitn(2, "==").collect();
    if parts.len() != 2 {
        return None;
    }
    let lhs = parts[0].trim();
    let rhs = parts[1].trim();
    if lhs == rhs && !lhs.is_empty() {
        Some(true)
    } else {
        None
    }
}

/// Parse `"IDENT == LITERAL"` or `"LITERAL == IDENT"`, where `IDENT` is a
/// single Rust-style identifier and `LITERAL` is an `i64` decimal literal.
/// Returns `(ident, literal)`. Used to resolve `#[sanctify::assume(...)]`
/// declarations and the invariants they bound.
#[cfg(feature = "smt")]
fn parse_symbolic_equality(expr: &str) -> Option<(String, i64)> {
    fn is_ident(s: &str) -> bool {
        let mut chars = s.chars();
        match chars.next() {
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
            _ => return false,
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    let expr = expr.trim();
    let parts: Vec<&str> = expr.splitn(2, "==").collect();
    if parts.len() != 2 {
        return None;
    }
    let lhs = parts[0].trim();
    let rhs = parts[1].trim();

    if let (true, Ok(lit)) = (is_ident(lhs), rhs.parse::<i64>()) {
        return Some((lhs.to_string(), lit));
    }
    if let (Ok(lit), true) = (lhs.parse::<i64>(), is_ident(rhs)) {
        return Some((rhs.to_string(), lit));
    }
    None
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_finds_sanctify_namespace_attribute() {
        let source = r#"
            use soroban_sdk::{contract, contractimpl, Env};

            #[contract]
            pub struct Token;

            #[sanctify::invariant(total_supply == sum_of_balances())]
            #[contractimpl]
            impl Token {
                pub fn total_supply(_env: Env) -> i128 { 0 }
            }
        "#;
        let decls = scan_invariant_attrs(source, "test.rs");
        assert_eq!(decls.len(), 1);
        assert!(decls[0].expr_str.contains("total_supply"));
        assert!(decls[0].location.contains("test.rs"));
    }

    #[test]
    fn test_scan_finds_short_form_attribute() {
        let source = r#"
            #[invariant(x == x)]
            impl MyContract {
                pub fn noop() {}
            }
        "#;
        let decls = scan_invariant_attrs(source, "contract.rs");
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].expr_str.trim(), "x == x");
    }

    #[test]
    fn test_scan_returns_empty_when_no_attribute() {
        let source = r#"
            #[contractimpl]
            impl Token {
                pub fn transfer(_env: soroban_sdk::Env) {}
            }
        "#;
        let decls = scan_invariant_attrs(source, "token.rs");
        assert!(decls.is_empty());
    }

    #[test]
    fn test_scan_multiple_invariants_on_separate_impls() {
        let source = r#"
            #[sanctify::invariant(a == b())]
            impl ContractA { pub fn a() {} }

            #[sanctify::invariant(c == d())]
            impl ContractB { pub fn c() {} }
        "#;
        let decls = scan_invariant_attrs(source, "multi.rs");
        assert_eq!(decls.len(), 2);
    }

    #[test]
    fn test_scan_invalid_syntax_returns_empty() {
        let decls = scan_invariant_attrs("this is not rust", "bad.rs");
        assert!(decls.is_empty());
    }

    #[cfg(feature = "smt")]
    #[test]
    fn test_smt_verifier_proves_integer_tautology() {
        let decl = InvariantDecl {
            contract_name: "Token".to_string(),
            expr_str: "42 == 42".to_string(),
            location: "test.rs:1".to_string(),
        };
        let result = SmtInvariantVerifier::new().verify_one(&decl);
        assert_eq!(result, InvariantVerifyResult::Proven);
    }

    #[cfg(feature = "smt")]
    #[test]
    fn test_smt_verifier_refutes_false_equality() {
        let decl = InvariantDecl {
            contract_name: "Token".to_string(),
            expr_str: "1 == 2".to_string(),
            location: "test.rs:1".to_string(),
        };
        let result = SmtInvariantVerifier::new().verify_one(&decl);
        assert!(matches!(result, InvariantVerifyResult::Refuted { .. }));
    }

    #[cfg(feature = "smt")]
    #[test]
    fn test_smt_verifier_unsupported_for_function_call() {
        let decl = InvariantDecl {
            contract_name: "Token".to_string(),
            expr_str: "total_supply() == sum_of_balances()".to_string(),
            location: "test.rs:1".to_string(),
        };
        let result = SmtInvariantVerifier::new().verify_one(&decl);
        assert_eq!(result, InvariantVerifyResult::Unsupported);
    }

    // ── assume/assert (issue #734) ───────────────────────────────────────

    #[test]
    fn test_scan_assume_finds_sanctify_namespace_attribute() {
        let source = r#"
            #[sanctify::assume(cap == 100)]
            #[sanctify::invariant(cap == 100)]
            impl Pool {
                pub fn cap() -> i128 { 0 }
            }
        "#;
        let assumes = scan_assume_attrs(source, "pool.rs");
        let invariants = scan_invariant_attrs(source, "pool.rs");
        assert_eq!(assumes.len(), 1);
        assert_eq!(invariants.len(), 1);
        assert_eq!(assumes[0].expr_str.trim(), "cap == 100");
    }

    #[test]
    fn test_scan_assume_ignores_non_assume_attrs() {
        let source = r#"
            #[sanctify::invariant(x == x)]
            impl Token { pub fn noop() {} }
        "#;
        assert!(scan_assume_attrs(source, "token.rs").is_empty());
    }

    /// Bounding example (acceptance criterion: "Example bounding a proof").
    ///
    /// `cap == 100` is a bare-identifier equality: on its own it is
    /// `Unsupported` (nothing pins `cap` to a value, so the SMT fast path
    /// can't decide it and would defer to Kani). Adding
    /// `#[sanctify::assume(cap == 100)]` bounds `cap` enough for the same
    /// invariant to be proven without ever invoking Kani.
    #[cfg(feature = "smt")]
    #[test]
    fn test_assume_bounds_an_otherwise_unsupported_invariant() {
        let decl = InvariantDecl {
            contract_name: "Pool".to_string(),
            expr_str: "cap == 100".to_string(),
            location: "pool.rs:3".to_string(),
        };
        let verifier = SmtInvariantVerifier::new();

        // Without the assumption, a bare identifier can't be decided.
        assert_eq!(
            verifier.verify_one(&decl),
            InvariantVerifyResult::Unsupported
        );

        // With a matching assumption scoped to the same contract, it's provable.
        let assumption = InvariantDecl {
            contract_name: "Pool".to_string(),
            expr_str: "cap == 100".to_string(),
            location: "pool.rs:2".to_string(),
        };
        assert_eq!(
            verifier.verify_one_with_assumptions(&decl, &[assumption]),
            InvariantVerifyResult::Proven
        );
    }

    #[cfg(feature = "smt")]
    #[test]
    fn test_assume_refutes_contradicted_invariant() {
        let decl = InvariantDecl {
            contract_name: "Pool".to_string(),
            expr_str: "cap == 100".to_string(),
            location: "pool.rs:3".to_string(),
        };
        let assumption = InvariantDecl {
            contract_name: "Pool".to_string(),
            expr_str: "cap == 50".to_string(),
            location: "pool.rs:2".to_string(),
        };
        let result = SmtInvariantVerifier::new().verify_one_with_assumptions(&decl, &[assumption]);
        assert!(matches!(result, InvariantVerifyResult::Refuted { .. }));
    }

    #[cfg(feature = "smt")]
    #[test]
    fn test_assume_from_a_different_contract_is_not_applied() {
        let decl = InvariantDecl {
            contract_name: "Pool".to_string(),
            expr_str: "cap == 100".to_string(),
            location: "pool.rs:3".to_string(),
        };
        let unrelated_assumption = InvariantDecl {
            contract_name: "Vault".to_string(),
            expr_str: "cap == 100".to_string(),
            location: "vault.rs:2".to_string(),
        };
        let result =
            SmtInvariantVerifier::new().verify_one_with_assumptions(&decl, &[unrelated_assumption]);
        assert_eq!(result, InvariantVerifyResult::Unsupported);
    }
}
