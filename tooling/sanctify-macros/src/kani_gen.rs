use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

/// Emit a `#[cfg(kani)] mod __sanctify_invariants_N { ... }` block containing
/// a single `#[kani::proof]` harness that asserts `expr`.
///
/// `impl_name` — the name of the `impl` block's self-type (used in the module
///               and function name so multiple invariants don't clash).
/// `expr`      — the invariant expression verbatim.
/// `index`     — zero-based ordinal when there are multiple invariants on the
///               same impl block.
/// `unwind`    — optional loop unwinding bound; when `Some(N)` the harness is
///               annotated with `#[kani::unwind(N)]` for bounded model checking.
///               Mirrors the `kani_unwind` key in `.sanctify.toml`.
///
/// The generated module uses `use super::*` so that all items from the
/// annotated `impl`'s module are in scope. Functions referenced by the
/// expression must be callable without a `soroban_sdk::Env` — follow the
/// pure-logic separation pattern from `contracts/kani-poc`.
pub fn kani_harness(
    impl_name: &str,
    expr: &TokenStream,
    index: usize,
    unwind: Option<u32>,
) -> TokenStream {
    let mod_name = Ident::new(
        &format!("__sanctify_inv_{}_{}", impl_name.to_lowercase(), index),
        Span::call_site(),
    );
    let fn_name = Ident::new(&format!("verify_invariant_{}", index), Span::call_site());
    let expr_str = expr.to_string();

    let unwind_attr = match unwind {
        Some(n) => quote! { #[kani::unwind(#n)] },
        None => quote! {},
    };

    quote! {
        #[cfg(kani)]
        #[allow(non_snake_case, dead_code)]
        mod #mod_name {
            use super::*;

            /// Auto-generated Kani proof harness for the invariant:
            ///
            #[doc = #expr_str]
            ///
            /// The invariant expression is inserted verbatim. For Kani to
            /// verify it, all functions referenced in the expression must
            /// operate on primitive types only (no soroban_sdk::Env). Follow
            /// the pure-logic separation pattern from contracts/kani-poc.
            #[kani::proof]
            #unwind_attr
            fn #fn_name() {
                assert!(#expr, "sanctify invariant violated: {}", stringify!(#expr));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    #[test]
    fn harness_without_unwind_omits_attribute() {
        let expr = quote! { x == x };
        let ts = kani_harness("Token", &expr, 0, None);
        let s = ts.to_string();
        assert!(!s.contains("kani :: unwind"), "expected no unwind attr, got: {s}");
        assert!(s.contains("kani :: proof"));
    }

    #[test]
    fn harness_with_unwind_emits_attribute() {
        let expr = quote! { x == x };
        let ts = kani_harness("Token", &expr, 0, Some(10));
        let s = ts.to_string();
        assert!(s.contains("kani :: unwind"), "expected unwind attr, got: {s}");
        assert!(s.contains("10"), "expected unwind value 10, got: {s}");
        assert!(s.contains("kani :: proof"));
    }

    #[test]
    fn harness_module_name_uses_impl_name() {
        let expr = quote! { true };
        let ts = kani_harness("MyContract", &expr, 2, None);
        let s = ts.to_string();
        assert!(s.contains("__sanctify_inv_mycontract_2"));
    }
}
