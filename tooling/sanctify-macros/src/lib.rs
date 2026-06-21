/// Reserved for future runtime-assertion mode — parses the invariant expression
/// with its original token stream so diagnostics can reference the source span.
#[allow(dead_code)]
mod invariant_args;
mod kani_gen;

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

    // Read the unwinding bound from the compile-time environment variable
    // `SANCTIFY_KANI_UNWIND`. The CLI sets this before invoking `cargo kani`
    // when `kani_unwind` is configured in `.sanctify.toml`.
    let unwind: Option<u32> = option_env!("SANCTIFY_KANI_UNWIND")
        .and_then(|s| s.parse().ok());

    let harness = kani_gen::kani_harness(&self_name, &args2, 0, unwind);

    let expanded = quote! {
        #impl_item
        #harness
    };

    expanded.into()
}
