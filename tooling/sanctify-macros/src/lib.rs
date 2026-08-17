mod ident_suffix;
/// Reserved for future runtime-assertion mode — parses the invariant expression
/// with its original token stream so diagnostics can reference the source span.
#[allow(dead_code)]
mod invariant_args;
mod kani_gen;
mod runtime_guard_gen;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse2, parse_macro_input, spanned::Spanned, Expr, ItemImpl};

/// Declare a contract-level invariant that Sanctifier will verify.
///
/// ## Usage
///
/// ```ignore
/// #[sanctify::invariant(total_supply == sum_of_balances())]
/// #[contractimpl]
/// impl Token { ... }
/// ```
///
/// In a normal build the attribute is transparent — it emits the `impl` block
/// unchanged so the Soroban toolchain sees exactly what it expects.
///
/// When compiled with `RUSTFLAGS="--cfg kani"` (i.e. under `cargo kani`) the
/// macro additionally emits a `#[kani::proof]` harness that asserts the
/// invariant expression. All functions referenced by the expression must be
/// callable without a `soroban_sdk::Env` — see the pure-logic separation
/// pattern in `contracts/kani-poc`.
///
/// In debug builds (or release with the `sanctify-runtime-invariants`
/// feature enabled), the macro also emits a runtime-checkable counterpart:
/// `Self::__sanctify_check_invariant_<suffix>(&env) -> bool`, which evaluates
/// the same expression and publishes an `inv_pass`/`inv_fail` event using
/// the exact schema `sanctifier_guards::guard_invariant!` uses. It does not
/// trap — call it at whichever point in your contract should enforce the
/// invariant, and decide what a `false` result means there.
///
/// `<suffix>` is derived deterministically from the invariant expression's
/// own text (see `ident_suffix::invariant_ident_suffix`) so that stacking
/// more than one `#[invariant(...)]` on the same impl block never collides
/// — each expression gets its own distinct generated method. It is not,
/// however, something to hand-compute: write `Self::__sanctify_check_invariant_`
/// with any placeholder tail and let the compiler's "no associated item
/// named ... did you mean" suggestion fill in the real name, the same way
/// you'd discover any other macro-generated identifier.
///
/// See `tooling/sanctify-macros/README.md` and
/// `tooling/sanctifier-guards/docs/telemetry-schema.md` for details. In a
/// release build without that feature, the method does not exist at all —
/// zero cost in production.
///
/// `sanctifier verify` scans source files for this attribute and dispatches
/// invariant expressions to the Z3 SMT backend where possible.
#[proc_macro_attribute]
pub fn invariant(args: TokenStream, input: TokenStream) -> TokenStream {
    let args2 = TokenStream2::from(args.clone());

    // Parse the argument as a Rust expression. The parsed `Expr` (not the
    // raw `TokenStream`) is what feeds both sibling generators below —
    // re-serializing through `syn`/`quote` gives a canonical stringification
    // that doesn't depend on how the original tokens were spaced, which
    // matters because their generated identifiers are derived from this
    // expression's text (see `ident_suffix::invariant_ident_suffix`).
    let parsed_expr: Expr = match parse2(args2.clone()) {
        Ok(expr) => expr,
        Err(e) => return e.to_compile_error().into(),
    };

    let impl_item: ItemImpl = parse_macro_input!(input as ItemImpl);

    // Derive the self-type name for stable module/function identifiers.
    let self_name = impl_item
        .self_ty
        .span()
        .source_text()
        .unwrap_or_else(|| "Contract".to_string());

    let harness = kani_gen::kani_harness(&self_name, &parsed_expr, 0);
    let runtime_guard =
        runtime_guard_gen::runtime_guard_impl(&impl_item, &self_name, &parsed_expr, 0);

    let expanded = quote! {
        #impl_item
        #harness
        #runtime_guard
    };

    expanded.into()
}
