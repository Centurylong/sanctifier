use serde::Serialize;

/// A single `#[sanctify::invariant(...)]` declaration found in source.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InvariantDecl {
    /// Name of the `impl` self-type the attribute was placed on.
    pub contract_name: String,
    /// The raw invariant expression as it appears in source.
    pub expr_str: String,
    /// Human-readable location string (`file:line`).
    pub location: String,
}

/// The outcome of attempting to verify one `InvariantDecl`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InvariantVerifyResult {
    /// The SMT solver proved the invariant holds for all inputs.
    Proven,
    /// The SMT solver found a counterexample (the invariant can be violated).
    Refuted { counterexample: String },
    /// The solver timed out or returned unknown.
    Unknown,
    /// The invariant expression is not in a form the SMT backend can check.
    Unsupported,
}
