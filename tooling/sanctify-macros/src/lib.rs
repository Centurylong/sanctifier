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

    // Multiple `#[invariant(...)]` attributes can be stacked on one impl
    // block (see contracts/sep41-token-invariants), and each is expanded as
    // an independent macro invocation — a fixed index (the previous
    // behavior) collided because every invocation generated the same
    // `__sanctify_check_invariant_0`. Attribute macros are expanded
    // top-to-bottom, and each invocation only sees the attributes still
    // unexpanded below it (the ones above it, including itself, are already
    // stripped by the time it runs) — so the count of remaining sibling
    // `#[invariant(...)]` attributes gives each one a distinct, deterministic
    // index, assigned bottom-up: the attribute closest to the impl item
    // (processed last) always gets index 0. That preserves the documented
    // `__sanctify_check_invariant_0` name — and every existing call site
    // that hardcodes it, e.g. contracts/token-invariants — for the common
    // case of a single invariant on an impl block, where there are no
    // remaining siblings to count.
    let index = impl_item
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("invariant"))
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
