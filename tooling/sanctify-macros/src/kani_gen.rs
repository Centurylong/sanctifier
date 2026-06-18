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
pub fn kani_harness(impl_name: &str, expr: &TokenStream, index: usize) -> TokenStream {
    let mod_name = Ident::new(
        &format!("__sanctify_inv_{}_{}", impl_name.to_lowercase(), index),
        Span::call_site(),
    );
    let fn_name = Ident::new(&format!("verify_invariant_{}", index), Span::call_site());
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
