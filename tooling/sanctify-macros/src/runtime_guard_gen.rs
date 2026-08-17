use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, ItemImpl};

/// Emit a sibling `impl` block containing a runtime-checkable counterpart of
/// the static `#[sanctify::invariant(...)]` expression.
///
/// The generated method, `__sanctify_check_invariant_N`, evaluates the same
/// invariant expression the static side verifies and publishes the exact
/// `inv_fail` / `inv_pass` event schema `sanctifier_guards::guard_invariant!`
/// publishes (topic `inv_fail`/`inv_pass`, data payload
/// `(symbol_short!("cond"), String)`) — so a monitor subscribed to that
/// schema sees violations caught this way too, with no second schema to
/// track. See `tooling/sanctifier-guards/docs/telemetry-schema.md` for the
/// full schema contract.
///
/// It returns `bool` rather than trapping. The macro decorates a whole
/// `impl` block, not a single method, so it has no way to know which
/// call site (or none) should abort on violation — that decision stays
/// with the contract author:
///
/// ```ignore
/// if !Self::__sanctify_check_invariant_0(&env) {
///     panic_with_error!(&env, Error::InvariantBroken);
/// }
/// ```
///
/// Same constraint as the Kani harness this sits alongside: the expression
/// must be callable without a `soroban_sdk::Env` (the pure-logic separation
/// pattern from `contracts/kani-poc`). `env` here is used only to publish
/// the telemetry event, not to evaluate the expression.
///
/// Compiled only in debug builds by default (`cfg(debug_assertions)`) — the
/// "toggle per build profile" from the issue — so it is exactly zero cost
/// in a release Soroban build. A consuming crate can force it on in release
/// too by declaring a `sanctify-runtime-invariants` feature and enabling it.
pub fn runtime_guard_impl(
    impl_item: &ItemImpl,
    expr: &TokenStream,
    disambiguator: u64,
) -> TokenStream {
    let self_ty = &impl_item.self_ty;
    let fn_name = Ident::new(
        &format!("__sanctify_check_invariant_{disambiguator:x}"),
        Span::call_site(),
    );
    let expr_str = expr.to_string();

    quote! {
        #[cfg(any(debug_assertions, feature = "sanctify-runtime-invariants"))]
        #[allow(non_snake_case, dead_code)]
        impl #self_ty {
            /// Runtime-checkable counterpart of the static invariant:
            #[doc = #expr_str]
            ///
            /// Evaluates the expression, publishes `inv_pass` or `inv_fail`
            /// on `env` using the same schema as
            /// `sanctifier_guards::guard_invariant!`, and returns whether it
            /// held. Does not trap; the caller decides what a `false`
            /// result means at the call site.
            pub fn #fn_name(env: &::soroban_sdk::Env) -> bool {
                let __sanctify_ok: bool = #expr;
                if __sanctify_ok {
                    env.events().publish(
                        (::soroban_sdk::symbol_short!("inv_pass"),),
                        (
                            ::soroban_sdk::symbol_short!("cond"),
                            ::soroban_sdk::String::from_str(env, #expr_str),
                        ),
                    );
                } else {
                    env.events().publish(
                        (::soroban_sdk::symbol_short!("inv_fail"),),
                        (
                            ::soroban_sdk::symbol_short!("cond"),
                            ::soroban_sdk::String::from_str(env, #expr_str),
                        ),
                    );
                }
                __sanctify_ok
            }
        }
    }
}
