//! Public-input encoding for the Sanctifier audit proof.
//!
//! # Field
//! All inputs are BLS12-381 scalar field elements (`Fr`), which are ~254-bit
//! integers in the range `[0, r)` where
//! `r = 0x73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001`.
//!
//! # Encoding
//!
//! | Input              | Description |
//! |--------------------|-------------|
//! | `wasm_hash`        | `sha256(wasm_bytes) mod r` — collapses a 256-bit SHA-256 digest into the scalar field; collision probability ≈ 2⁻¹²⁶. |
//! | `ruleset_version`  | `Fr::from(version as u64)` — monotonically increasing u32 (1 = current 11-rule set). |
//! | `score_threshold`  | `Fr::from(threshold as u64)` — integer in `[0, N_RULES]`. |
//! | `rules_commitment` | `Poseidon(r₀, r₁, …, r_{N-1})` using the fixed [`POSEIDON_SEED`]-derived parameters; single `Fr` output. |
//!
//! The on-chain verifier **must** use the same field, curve, and Poseidon
//! parameters to validate a proof.

use ark_bls12_381::Fr;
use ark_crypto_primitives::sponge::{
    poseidon::{PoseidonConfig, PoseidonSponge},
    CryptographicSponge,
};
use ark_ff::{One, PrimeField, Zero};
use sha2::{Digest, Sha256};

use crate::{params::poseidon_config, N_RULES};

/// Encode raw WASM bytes as a field element: `sha256(wasm_bytes) mod r`.
pub fn wasm_hash_field(wasm_bytes: &[u8]) -> Fr {
    let digest = Sha256::digest(wasm_bytes);
    Fr::from_be_bytes_mod_order(&digest)
}

/// Encode a ruleset version number as a field element.
pub fn ruleset_version_field(version: u32) -> Fr {
    Fr::from(version as u64)
}

/// Encode a score threshold as a field element.
pub fn score_threshold_field(threshold: u64) -> Fr {
    Fr::from(threshold)
}

/// Compute the Poseidon commitment over the per-rule boolean results.
///
/// Each `rule_result[i]` is `true` (pass) or `false` (fail); the
/// function maps these to `Fr::one()` and `Fr::zero()` before hashing.
pub fn rules_commitment(rule_results: &[bool; N_RULES]) -> Fr {
    let cfg: PoseidonConfig<Fr> = poseidon_config();
    let mut sponge = PoseidonSponge::<Fr>::new(&cfg);
    let elems: Vec<Fr> = rule_results
        .iter()
        .map(|&b| if b { Fr::one() } else { Fr::zero() })
        .collect();
    sponge.absorb(&elems);
    sponge.squeeze_field_elements::<Fr>(1)[0]
}
