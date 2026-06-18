use proc_macro2::TokenStream;
use syn::{parse::Parse, parse::ParseStream, Expr, Result};

/// The token payload of `#[sanctify::invariant(EXPR)]`.
///
/// Parsed as a single Rust expression so that any valid expression — equality,
/// boolean, function call — is accepted without special-casing.
pub struct InvariantArgs {
    pub expr: Expr,
    /// Original token string, kept for diagnostic messages and comment generation.
    pub expr_tokens: TokenStream,
}

impl Parse for InvariantArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let expr_tokens: TokenStream = input.fork().parse()?;
        let expr: Expr = input.parse()?;
        Ok(InvariantArgs { expr, expr_tokens })
    }
}
