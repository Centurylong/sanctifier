use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Expr, Ident};

use crate::ident_suffix::invariant_ident_suffix;

/// Emit a `#[cfg(kani)] mod __sanctify_invariants_N { ... }` block containing
/// a single `#[kani::proof]` harness that asserts `expr`.
///
/// `impl_name` — the name of the `impl` block's self-type (used in the module
///               and function name so multiple invariants don't clash).
/// `expr`      — the invariant expression verbatim.
/// `index`     — zero-based ordinal when there are multiple invariants on the
///               same impl block. Each `#[invariant(...)]` attribute expands
///               independently with no visibility into sibling attributes on
///               the same impl, so this is always 0 in practice today; the
///               generated name's real uniqueness comes from hashing `expr`
///               itself (see `invariant_ident_suffix`), which is what
///               actually distinguishes multiple stacked invariants on one
///               impl block. `index` is kept for future use if that ever
///               changes.
///
/// The generated module uses `use super::*` so that all items from the
/// annotated `impl`'s module are in scope. Functions referenced by the
/// expression must be callable without a `soroban_sdk::Env` — follow the
/// pure-logic separation pattern from `contracts/kani-poc`.
pub fn kani_harness(impl_name: &str, expr: &Expr, index: usize) -> TokenStream {
    let suffix = invariant_ident_suffix(impl_name, expr, index);
    let mod_name = Ident::new(
        &format!("__sanctify_inv_{}_{}", impl_name.to_lowercase(), suffix),
        Span::call_site(),
    );
    let fn_name = Ident::new(&format!("verify_invariant_{suffix}"), Span::call_site());
    let expr_str = quote!(#expr).to_string();

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
