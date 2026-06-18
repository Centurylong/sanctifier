use serde::Serialize;
use syn::{spanned::Spanned, visit::Visit, Attribute, File, ItemImpl};

/// Walk a parsed `File` and collect every `#[sanctify::invariant(...)]` found
/// on `impl` blocks.
pub fn scan_invariant_attrs(source: &str, file_label: &str) -> Vec<InvariantDecl> {
    let ast: File = match syn::parse_str(source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut visitor = InvariantVisitor {
        decls: Vec::new(),
        file_label: file_label.to_string(),
    };
    visitor.visit_file(&ast);
    visitor.decls
}

struct InvariantVisitor {
    decls: Vec<InvariantDecl>,
    file_label: String,
}

impl<'ast> Visit<'ast> for InvariantVisitor {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        for attr in &node.attrs {
            if let Some(expr_str) = extract_invariant_expr(attr) {
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

/// Return the expression string if `attr` is `#[sanctify::invariant(...)]` or
/// `#[invariant(...)]`, otherwise `None`.
fn extract_invariant_expr(attr: &Attribute) -> Option<String> {
    let path = attr.path();
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    let is_invariant = match segments.as_slice() {
        [name] => name == "invariant",
        [ns, name] => ns == "sanctify" && name == "invariant",
        _ => false,
    };

    if !is_invariant {
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
#[derive(Debug, Clone, Serialize)]
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
impl SmtInvariantVerifier {
    pub fn new() -> Self {
        SmtInvariantVerifier
    }

    /// Try to verify a single invariant declaration.
    pub fn verify_one(&self, decl: &InvariantDecl) -> InvariantVerifyResult {
        use z3::ast::Int;
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
}

/// Parse `"N == M"` where N and M are i64 decimal literals.
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
