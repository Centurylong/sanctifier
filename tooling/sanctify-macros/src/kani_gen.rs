use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

/// Emit a `#[cfg(kani)] mod __sanctify_invariants_N { ... }` block containing
/// a single `#[kani::proof]` harness that asserts `expr`.
///
/// `impl_name`     — the name of the `impl` block's self-type (used in the
///                    module and function name so multiple invariants don't
///                    clash).
/// `expr`          — the invariant expression verbatim.
/// `disambiguator` — a value that's stable for a given invariant but differs
///                    across the invariants on the same impl block, so their
///                    generated names don't collide. Each `#[invariant(...)]`
///                    attribute expands independently — it has no visibility
///                    into a shared ordinal counter — so callers derive this
///                    from the invariant expression itself (see `invariant`
///                    in `lib.rs`), not from a sequential index.
///
/// The generated module uses `use super::*` so that all items from the
/// annotated `impl`'s module are in scope. Functions referenced by the
/// expression must be callable without a `soroban_sdk::Env` — follow the
/// pure-logic separation pattern from `contracts/kani-poc`.
pub fn kani_harness(impl_name: &str, expr: &TokenStream, disambiguator: u64) -> TokenStream {
    let mod_name = Ident::new(
        &format!(
            "__sanctify_inv_{}_{:x}",
            impl_name.to_lowercase(),
            disambiguator
        ),
        Span::call_site(),
    );
    let fn_name = Ident::new(
        &format!("verify_invariant_{:x}", disambiguator),
        Span::call_site(),
    );
    let expr_str = expr.to_string();

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
            fn #fn_name() {
                assert!(#expr, "sanctify invariant violated: {}", stringify!(#expr));
            }
        }
    }
}
