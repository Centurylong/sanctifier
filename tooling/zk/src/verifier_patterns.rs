//! Recognizes which zero-knowledge proof system a Soroban verifier contract
//! is built on, from its source text.
//!
//! `sanctifier-zk`'s circuit/encoding/params modules are specific to this
//! crate's own Groth16 audit-proof pipeline. Contracts that *sanctifier
//! analyzes* aren't limited to Groth16, though — PLONK and Halo2 are
//! increasingly common because they don't require a per-circuit trusted
//! setup ceremony. This module gives the rest of the toolchain a shared,
//! lightweight way to classify which family a verifier contract belongs to,
//! so detectors and docs can reason about "any verifier", not just Groth16
//! ones.

/// A zero-knowledge proof system a verifier contract may be built on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofSystem {
    /// Groth16 (BLS12-381/BN254) — requires a per-circuit trusted setup.
    Groth16,
    /// PLONK — universal (not per-circuit) trusted setup.
    Plonk,
    /// Halo2 — transparent setup, no trusted ceremony at all.
    Halo2,
}

impl ProofSystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProofSystem::Groth16 => "groth16",
            ProofSystem::Plonk => "plonk",
            ProofSystem::Halo2 => "halo2",
        }
    }

    /// Whether this proof system needs a per-circuit trusted setup ceremony.
    pub fn requires_trusted_setup(&self) -> bool {
        matches!(self, ProofSystem::Groth16)
    }
}

/// Crate/type signatures characteristic of each proof system's Rust
/// verifier implementations.
const GROTH16_MARKERS: &[&str] = &["ark_groth16", "ark-groth16", "Groth16::", "VerifyingKey"];
const PLONK_MARKERS: &[&str] = &["ark_plonk", "ark-plonk", "plonky2", "PlonkVerifier", "plonk::"];
const HALO2_MARKERS: &[&str] = &["halo2_proofs", "halo2-proofs", "halo2curves", "halo2_gadgets"];

/// Best-effort detection of every proof system referenced in `source`.
///
/// This is a source-text heuristic (crate names / type paths), not a
/// semantic analysis — it's meant to let detectors and reports say "this
/// looks like a PLONK verifier" without hand-listing every contract, not to
/// prove which cryptographic scheme is actually linked in.
pub fn detect_proof_systems(source: &str) -> Vec<ProofSystem> {
    let mut found = Vec::new();
    if GROTH16_MARKERS.iter().any(|m| source.contains(m)) {
        found.push(ProofSystem::Groth16);
    }
    if PLONK_MARKERS.iter().any(|m| source.contains(m)) {
        found.push(ProofSystem::Plonk);
    }
    if HALO2_MARKERS.iter().any(|m| source.contains(m)) {
        found.push(ProofSystem::Halo2);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_groth16_verifier() {
        let source = r#"
            use ark_groth16::{Groth16, Proof, VerifyingKey};
            fn verify() -> bool { Groth16::verify_with_processed_vk(&pvk, &inputs, &proof).unwrap_or(false) }
        "#;
        assert_eq!(detect_proof_systems(source), vec![ProofSystem::Groth16]);
        assert!(ProofSystem::Groth16.requires_trusted_setup());
    }

    #[test]
    fn detects_halo2_verifier() {
        let source = r#"
            use halo2_proofs::plonk::verify_proof;
        "#;
        let found = detect_proof_systems(source);
        assert!(found.contains(&ProofSystem::Halo2));
        assert!(!ProofSystem::Halo2.requires_trusted_setup());
    }

    #[test]
    fn detects_plonk_verifier() {
        let source = r#"
            use ark_plonk::proof_system::verifier::Verifier;
        "#;
        assert_eq!(detect_proof_systems(source), vec![ProofSystem::Plonk]);
    }

    #[test]
    fn returns_empty_for_no_zk_code() {
        let source = "pub fn transfer() {}";
        assert!(detect_proof_systems(source).is_empty());
    }
}
