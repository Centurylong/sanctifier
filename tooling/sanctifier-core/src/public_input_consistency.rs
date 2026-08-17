//! Circuit-vs-contract public-input consistency (issue #740).
//!
//! A ZK verifier contract and the circuit its proofs were generated against
//! must agree on the public inputs: how many there are, and (ideally) their
//! order. Soroban contracts don't run the circuit, so nothing enforces that
//! agreement automatically — a contract that hardcodes "4 public inputs"
//! silently goes stale the moment someone adds a 5th field to the circuit's
//! public-input struct and forgets to update the verifier. See
//! `docs/public-input-consistency.md` for the feasibility write-up this
//! module implements (approach, scope, and what's deliberately left out).
//!
//! This is a static, source-text comparison — same technique as every rule
//! in `rules/`: parse both files with `syn`, count structural signals, and
//! compare. It does not run the circuit or the contract.

use serde::Serialize;
use syn::visit::Visit;
use syn::{parse_str, ExprBinary, ExprCall, ExprMethodCall, File};

/// What the circuit side declares as its public-input count, inferred by
/// counting `FpVar::new_input(...)` (or any `..::new_input(...)`) call sites
/// in `generate_constraints`-style circuit source. Each one allocates exactly
/// one public input, in the order it's called — see
/// `ark-relations`/`ark-r1cs-std`'s `AllocVar::new_input`.
pub fn circuit_public_input_count(source: &str) -> Option<usize> {
    let file: File = parse_str(source).ok()?;
    let mut visitor = NewInputCounter { count: 0 };
    visitor.visit_file(&file);
    if visitor.count == 0 {
        None
    } else {
        Some(visitor.count)
    }
}

/// What the contract side assumes as its public-input count, inferred from a
/// `<count> * 32` pattern (32 bytes being one compressed BLS12-381 `Fr`
/// element) — the shape `contracts/zk-verifier/src/lib.rs` uses to validate
/// `public_inputs_bytes.len()` before deserializing. Picks the first such
/// pattern found; a contract with more than one is out of scope (see the
/// docs page's "Scope" section).
pub fn contract_public_input_count(source: &str) -> Option<usize> {
    let file: File = parse_str(source).ok()?;
    let mut visitor = FieldElementCountFinder {
        element_size_bytes: 32,
        count: None,
    };
    visitor.visit_file(&file);
    visitor.count
}

/// Result of comparing a circuit's declared public-input count against a
/// contract's assumed one.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PublicInputConsistencyReport {
    pub circuit_count: Option<usize>,
    pub contract_count: Option<usize>,
    /// `true` when both sides were found and they disagree.
    pub mismatch: bool,
}

impl PublicInputConsistencyReport {
    pub fn message(&self) -> String {
        match (self.circuit_count, self.contract_count) {
            (Some(c), Some(k)) if c == k => {
                format!("circuit and contract agree on {c} public input(s).")
            }
            (Some(c), Some(k)) => format!(
                "MISMATCH: circuit declares {c} public input(s) (via new_input calls) but the \
                 contract assumes {k} (via a `N * 32`-byte length check). A verifier built \
                 against one will reject — or worse, silently misparse — proofs shaped for the \
                 other."
            ),
            (None, _) => "could not determine the circuit's public-input count (no \
                           `..::new_input(...)` call sites found)."
                .to_string(),
            (Some(_), None) => "could not determine the contract's assumed public-input count \
                                 (no `N * 32`-byte length check found)."
                .to_string(),
        }
    }
}

/// Compare a circuit source file against a contract source file.
pub fn check_consistency(
    circuit_source: &str,
    contract_source: &str,
) -> PublicInputConsistencyReport {
    let circuit_count = circuit_public_input_count(circuit_source);
    let contract_count = contract_public_input_count(contract_source);
    let mismatch = matches!((circuit_count, contract_count), (Some(c), Some(k)) if c != k);
    PublicInputConsistencyReport {
        circuit_count,
        contract_count,
        mismatch,
    }
}

struct NewInputCounter {
    count: usize,
}

impl<'ast> Visit<'ast> for NewInputCounter {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let syn::Expr::Path(p) = node.func.as_ref() {
            if p.path.segments.last().map(|s| s.ident == "new_input") == Some(true) {
                self.count += 1;
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        if node.method == "new_input" {
            self.count += 1;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

struct FieldElementCountFinder {
    element_size_bytes: i64,
    count: Option<usize>,
}

impl<'ast> Visit<'ast> for FieldElementCountFinder {
    fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
        if self.count.is_none() {
            if let syn::BinOp::Mul(_) = node.op {
                let lhs = as_int_literal(&node.left);
                let rhs = as_int_literal(&node.right);
                self.count = match (lhs, rhs) {
                    (Some(n), Some(sz)) if sz == self.element_size_bytes && n > 0 => {
                        Some(n as usize)
                    }
                    (Some(sz), Some(n)) if sz == self.element_size_bytes && n > 0 => {
                        Some(n as usize)
                    }
                    _ => None,
                };
            }
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

fn as_int_literal(expr: &syn::Expr) -> Option<i64> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(lit_int),
        ..
    }) = expr
    {
        lit_int.base10_parse::<i64>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CIRCUIT_FOUR_INPUTS: &str = r#"
        fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
            let wasm_hash_var = FpVar::new_input(ns!(cs, "wasm_hash"), || Ok(self.public.wasm_hash))?;
            let ruleset_ver_var = FpVar::new_input(ns!(cs, "ruleset_version"), || Ok(self.public.ruleset_version))?;
            let score_threshold_var = FpVar::new_input(ns!(cs, "score_threshold"), || Ok(self.public.score_threshold))?;
            let rules_commitment_var = FpVar::new_input(ns!(cs, "rules_commitment"), || Ok(self.public.rules_commitment))?;
            Ok(())
        }
    "#;

    const CONTRACT_FOUR_INPUTS: &str = r#"
        pub fn verify(env: Env, proof_bytes: Bytes, public_inputs_bytes: Bytes) -> bool {
            if public_inputs_bytes.len() != 4 * 32 {
                return false;
            }
            true
        }
    "#;

    #[test]
    fn counts_new_input_call_sites() {
        assert_eq!(circuit_public_input_count(CIRCUIT_FOUR_INPUTS), Some(4));
    }

    #[test]
    fn extracts_element_count_from_byte_length_check() {
        assert_eq!(contract_public_input_count(CONTRACT_FOUR_INPUTS), Some(4));
    }

    #[test]
    fn reports_no_mismatch_when_counts_agree() {
        let report = check_consistency(CIRCUIT_FOUR_INPUTS, CONTRACT_FOUR_INPUTS);
        assert!(!report.mismatch);
        assert_eq!(report.circuit_count, Some(4));
        assert_eq!(report.contract_count, Some(4));
    }

    /// Prototype flags a mismatch (acceptance criterion): drop one public
    /// input from the circuit side without touching the contract's
    /// `4 * 32` check, and the report must catch it.
    #[test]
    fn flags_a_mismatch_when_circuit_drops_a_public_input() {
        let circuit_three_inputs = r#"
            fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
                let wasm_hash_var = FpVar::new_input(ns!(cs, "wasm_hash"), || Ok(self.public.wasm_hash))?;
                let score_threshold_var = FpVar::new_input(ns!(cs, "score_threshold"), || Ok(self.public.score_threshold))?;
                let rules_commitment_var = FpVar::new_input(ns!(cs, "rules_commitment"), || Ok(self.public.rules_commitment))?;
                Ok(())
            }
        "#;
        let report = check_consistency(circuit_three_inputs, CONTRACT_FOUR_INPUTS);
        assert!(report.mismatch);
        assert_eq!(report.circuit_count, Some(3));
        assert_eq!(report.contract_count, Some(4));
        assert!(report.message().contains("MISMATCH"));
    }

    #[test]
    fn returns_none_when_no_signal_present() {
        assert_eq!(circuit_public_input_count("fn f() {}"), None);
        assert_eq!(contract_public_input_count("fn f() {}"), None);
    }

    /// Grounds the checker in this repo's actual circuit and contract: as of
    /// this commit they agree (both 4), so this must report no mismatch.
    /// If a future change to `tooling/zk/src/circuit.rs`'s public inputs
    /// isn't mirrored in `contracts/zk-verifier/src/lib.rs` (or vice versa),
    /// this test starts failing — which is the point.
    #[test]
    fn the_repos_own_circuit_and_contract_currently_agree() {
        let circuit_source = include_str!("../../zk/src/circuit.rs");
        let contract_source = include_str!("../../../contracts/zk-verifier/src/lib.rs");
        let report = check_consistency(circuit_source, contract_source);
        assert_eq!(report.circuit_count, Some(4));
        assert_eq!(report.contract_count, Some(4));
        assert!(!report.mismatch, "{}", report.message());
    }
}
