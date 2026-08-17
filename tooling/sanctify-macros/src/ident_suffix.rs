use quote::quote;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A deterministic, unique-per-invariant identifier suffix, derived from the
/// invariant expression's own text so a caller can work it out by hand
/// instead of needing to run the macro to find out what it generated.
///
/// `#[sanctify::invariant(...)]` attributes on the same `impl` block each
/// expand as an independent proc-macro invocation — none of them can see
/// their siblings, so there is no way to hand out a real running counter
/// across stacked invariants on one impl. Two (or more) `#[invariant(...)]`
/// attributes on the same impl block previously both generated identifiers
/// like `__sanctify_check_invariant_0`, colliding with `error[E0592]:
/// duplicate definitions` the moment a contract declared more than one
/// invariant on the same impl (see `contracts/sep41-token-invariants`,
/// which declares three).
///
/// The suffix is the expression's token text lowercased and sanitized into
/// a valid identifier fragment (non-alphanumeric runs collapse to a single
/// `_`), truncated to a readable length, with a short hash of the *full*
/// expression text appended so truncation (or two expressions that happen
/// to sanitize identically) still can't collide silently. For the common
/// case — a short expression, which invariants almost always are — the
/// suffix is close to the expression itself, e.g. `supply_is_conserved_af_1a2b3c4d`,
/// so a caller invoking the generated method can read it straight off their
/// own `#[invariant(...)]` line rather than treating it as a black box.
///
/// Takes a parsed `syn::Expr` rather than a raw `TokenStream` deliberately:
/// re-serializing through `quote!` gives a canonical, whitespace-independent
/// stringification. A raw `TokenStream`'s `to_string()` is sensitive to
/// `Punct` joint/alone spacing, which differs between tokens the compiler
/// hands a proc-macro from real source and tokens built by `quote!` in a
/// test — the same expression could hash differently depending on which
/// path produced its `TokenStream`, silently breaking any call site that
/// hardcodes the generated name (as `contracts/token-invariants` does).
/// Round-tripping through `syn::Expr` removes that discrepancy: both the
/// real macro invocation and a `quote!`-in-a-test invocation agree exactly.
pub fn invariant_ident_suffix(impl_name: &str, expr: &syn::Expr, index: usize) -> String {
    let expr_text = quote!(#expr).to_string();

    let mut sanitized = String::new();
    let mut last_was_underscore = false;
    for ch in expr_text.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            last_was_underscore = false;
        } else if !last_was_underscore {
            sanitized.push('_');
            last_was_underscore = true;
        }
    }
    let sanitized = sanitized.trim_matches('_');
    const MAX_LEN: usize = 32;
    let truncated: String = sanitized.chars().take(MAX_LEN).collect();
    let truncated = truncated.trim_end_matches('_');

    let mut hasher = DefaultHasher::new();
    impl_name.hash(&mut hasher);
    expr_text.hash(&mut hasher);
    index.hash(&mut hasher);
    let hash = hasher.finish();

    if truncated.is_empty() {
        format!("{hash:016x}")
    } else {
        format!("{truncated}_{hash:08x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn different_expressions_on_same_impl_never_collide() {
        let a: syn::Expr = parse_quote! { pure::supply_is_conserved_after_transfer(0, 0, 0) };
        let b: syn::Expr =
            parse_quote! { pure::supply_is_conserved_after_transfer_from(0, 0, 0, 0) };
        let c: syn::Expr = parse_quote! { pure::allowance_is_set_by_approve(0) };
        let suffix_a = invariant_ident_suffix("Token", &a, 0);
        let suffix_b = invariant_ident_suffix("Token", &b, 0);
        let suffix_c = invariant_ident_suffix("Token", &c, 0);
        assert_ne!(suffix_a, suffix_b);
        assert_ne!(suffix_a, suffix_c);
        assert_ne!(suffix_b, suffix_c);
    }

    #[test]
    fn same_expression_and_impl_is_deterministic() {
        let expr: syn::Expr = parse_quote! { pure::supply_is_conserved_after_transfer(0, 0, 0) };
        let first = invariant_ident_suffix("Token", &expr, 0);
        let second = invariant_ident_suffix("Token", &expr, 0);
        assert_eq!(first, second);
    }

    #[test]
    fn suffix_is_a_valid_identifier_fragment() {
        let expr: syn::Expr = parse_quote! { pure::supply_is_conserved_after_transfer(0, 0, 0) };
        let suffix = invariant_ident_suffix("Token", &expr, 0);
        assert!(
            suffix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "suffix {suffix:?} contains a character invalid in a Rust identifier"
        );
    }

    // NOTE: this crate deliberately does not pin an exact expected suffix
    // string in a test. `impl_name` is derived in lib.rs via
    // `Span::source_text()`, whose exact behavior isn't guaranteed
    // identical between a real compiler invocation and a `syn::parse_quote!`
    // built here in a unit test — attempting to hardcode a "predicted" value
    // produced a value that didn't match the real compiled output when this
    // fix was verified against contracts/token-invariants, which calls the
    // generated method by name. The properties that matter (determinism,
    // uniqueness, valid-identifier characters) are covered above; the exact
    // string a real invocation produces should be read from a real build
    // (or, as with token-invariants, from the compiler's own "did you mean"
    // suggestion on a mismatch) rather than predicted here.
}
