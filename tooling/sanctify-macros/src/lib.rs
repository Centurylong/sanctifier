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
/// `Self::__sanctify_check_invariant_N(&env) -> bool`, which evaluates the
/// same expression and publishes an `inv_pass`/`inv_fail` event using the
/// exact schema `sanctifier_guards::guard_invariant!` uses. It does not trap
/// — call it at whichever point in your contract should enforce the
/// invariant, and decide what a `false` result means there. See
/// `tooling/sanctify-macros/README.md` and
/// `tooling/sanctifier-guards/docs/telemetry-schema.md` for details. In a
/// release build without that feature, the method does not exist at all —
/// zero cost in production.
///
/// `sanctifier verify` scans source files for this attribute and dispatches
/// invariant expressions to the Z3 SMT backend where possible.
#[proc_macro_attribute]
pub fn invariant(args: TokenStream, input: TokenStream) -> TokenStream {
    let args2 = TokenStream2::from(args.clone());

    // Validate the argument is a parseable Rust expression.
    if let Err(e) = parse2::<Expr>(args2.clone()) {
        return e.to_compile_error().into();
    }

    let impl_item: ItemImpl = parse_macro_input!(input as ItemImpl);

    // Derive the self-type name for stable module/function identifiers.
    let self_name = impl_item
        .self_ty
        .span()
        .source_text()
        .unwrap_or_else(|| "Contract".to_string());

    // When multiple `#[invariant(...)]` attributes are stacked on the same
    // `impl` block (the documented usage pattern above), Rust expands
    // stacked attribute macros one at a time, top-to-bottom: each
    // invocation's `input` still carries whichever sibling `#[invariant]`
    // attributes haven't been expanded yet (they sit unprocessed in
    // `impl_item.attrs` until the compiler works its way down to them).
    // Counting those remaining siblings gives every invocation on the same
    // impl block a distinct index (N-1, N-2, ..., 0 for N stacked
    // invariants), so the generated `__sanctify_check_invariant_N` /
    // `verify_invariant_N` names never collide. A single, unstacked
    // `#[invariant(...)]` always sees zero remaining siblings and keeps
    // index 0, unchanged from before.
    let index = impl_item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("invariant"))
        .count();

    let harness = kani_gen::kani_harness(&self_name, &args2, index);
    let runtime_guard = runtime_guard_gen::runtime_guard_impl(&impl_item, &args2, index);

    let expanded = quote! {
        #impl_item
        #harness
        #runtime_guard
    };

    expanded.into()
}
