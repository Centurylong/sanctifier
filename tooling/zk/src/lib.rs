//! `sanctifier-zk` — zero-knowledge audit proofs for Sanctifier.
//!
//! Proves in zero-knowledge that a WASM bytecode hash achieved at least a
//! target score under a given ruleset, without revealing the per-rule
//! findings.
//!
//! # Quick start
//! ```rust,ignore
//! use sanctifier_zk::{prove, verify, AuditStatement};
//! use ark_bls12_381::Fr;
//!
//! let stmt = AuditStatement {
//!     wasm_hash:        encoding::wasm_hash_field(wasm_bytes),
//!     ruleset_version:  encoding::ruleset_version_field(1),
//!     score_threshold:  encoding::score_threshold_field(9),
//!     rules_commitment: encoding::rules_commitment(&rule_results),
//! };
//! let proof = prove(&stmt, &rule_results, &pk)?;
//! assert!(verify(&stmt, &proof, &vk)?);
//! ```

pub mod circuit;
pub mod encoding;
pub mod params;

pub use circuit::{AuditCircuit, AuditPublicInputs, AuditWitness};

use anyhow::Result;
use ark_bls12_381::{Bls12_381, Fr};
use ark_ff::{BigInteger, PrimeField};
use ark_groth16::{Groth16, PreparedVerifyingKey, ProvingKey, VerifyingKey};
use ark_relations::r1cs::{ConstraintSystem, ConstraintSynthesizer};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_snark::{CircuitSpecificSetupSNARK, SNARK};
use ark_std::rand::{CryptoRng, RngCore};

/// Number of rules in the current sanctifier-core ruleset (v1).
pub const N_RULES: usize = 11;

/// Bits used to represent the max possible score difference.
/// `N_RULES = 11 < 2^4 = 16`, so 4 bits suffice; we use 8 for headroom.
pub const SCORE_BITS: usize = 8;

/// The public statement that is embedded in every proof.
pub type AuditStatement = AuditPublicInputs<Fr>;

// ── Key generation ────────────────────────────────────────────────────────────

/// Generate a circuit-specific proving / verifying key pair.
///
/// Call once per circuit shape; the keys are reusable for any number of proofs
/// over the same circuit (same `N_RULES`, same Poseidon parameters).
pub fn setup(rng: &mut (impl RngCore + CryptoRng)) -> Result<(ProvingKey<Bls12_381>, VerifyingKey<Bls12_381>)> {
    let circuit = dummy_circuit();
    let (pk, vk) = Groth16::<Bls12_381>::circuit_specific_setup(circuit, rng)
        .map_err(|e| anyhow::anyhow!("setup failed: {:?}", e))?;
    Ok((pk, vk))
}

// ── Proving ───────────────────────────────────────────────────────────────────

/// Generate a Groth16 proof for the given statement and witness.
///
/// Returns an error if `rule_results` does not satisfy the threshold encoded
/// in `stmt.score_threshold`.
pub fn prove(
    stmt: &AuditStatement,
    rule_results: &[bool; N_RULES],
    pk: &ProvingKey<Bls12_381>,
    rng: &mut (impl RngCore + CryptoRng),
) -> Result<ark_groth16::Proof<Bls12_381>> {
    // Pre-check: the prover must actually satisfy the threshold
    let score = rule_results.iter().filter(|&&b| b).count() as u64;
    let threshold = stmt.score_threshold.into_bigint().as_ref()[0];
    if score < threshold {
        anyhow::bail!(
            "score {score} is below threshold {threshold}; cannot produce a valid proof"
        );
    }

    let circuit = AuditCircuit {
        public: stmt.clone(),
        witness: Some(AuditWitness {
            rule_results: *rule_results,
        }),
        poseidon_params: params::poseidon_config(),
    };

    Groth16::<Bls12_381>::prove(pk, circuit, rng)
        .map_err(|e| anyhow::anyhow!("prove failed: {:?}", e))
}

// ── Verification ──────────────────────────────────────────────────────────────

/// Verify a Groth16 proof against a public statement.
pub fn verify(
    stmt: &AuditStatement,
    proof: &ark_groth16::Proof<Bls12_381>,
    vk: &VerifyingKey<Bls12_381>,
) -> Result<bool> {
    let pvk = Groth16::<Bls12_381>::process_vk(vk)
        .map_err(|e| anyhow::anyhow!("process_vk failed: {:?}", e))?;
    verify_with_pvk(stmt, proof, &pvk)
}

/// Verify using a pre-processed verifying key (faster when verifying many proofs).
pub fn verify_with_pvk(
    stmt: &AuditStatement,
    proof: &ark_groth16::Proof<Bls12_381>,
    pvk: &PreparedVerifyingKey<Bls12_381>,
) -> Result<bool> {
    let public_inputs = public_inputs_vec(stmt);
    Groth16::<Bls12_381>::verify_with_processed_vk(pvk, &public_inputs, proof)
        .map_err(|e| anyhow::anyhow!("verify failed: {:?}", e))
}

// ── Serialisation ─────────────────────────────────────────────────────────────

/// Serialise a proof to compressed bytes.
pub fn proof_to_bytes(proof: &ark_groth16::Proof<Bls12_381>) -> Vec<u8> {
    let mut buf = Vec::new();
    proof.serialize_compressed(&mut buf).expect("serialisation cannot fail");
    buf
}

/// Deserialise a proof from compressed bytes.
pub fn proof_from_bytes(bytes: &[u8]) -> Result<ark_groth16::Proof<Bls12_381>> {
    ark_groth16::Proof::<Bls12_381>::deserialize_compressed(bytes)
        .map_err(|e| anyhow::anyhow!("deserialise proof: {:?}", e))
}

// ── Constraint count (diagnostics) ────────────────────────────────────────────

/// Return the number of R1CS constraints in the circuit (useful for benchmarking).
pub fn constraint_count() -> usize {
    let cs = ConstraintSystem::<Fr>::new_ref();
    dummy_circuit()
        .generate_constraints(cs.clone())
        .expect("constraint generation failed");
    cs.num_constraints()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn dummy_circuit() -> AuditCircuit<Fr> {
    AuditCircuit {
        public: AuditPublicInputs {
            wasm_hash: Fr::from(0u64),
            ruleset_version: Fr::from(1u64),
            score_threshold: Fr::from(0u64),
            rules_commitment: Fr::from(0u64),
        },
        witness: None,
        poseidon_params: params::poseidon_config(),
    }
}

fn public_inputs_vec(stmt: &AuditStatement) -> Vec<Fr> {
    vec![
        stmt.wasm_hash,
        stmt.ruleset_version,
        stmt.score_threshold,
        stmt.rules_commitment,
    ]
}
