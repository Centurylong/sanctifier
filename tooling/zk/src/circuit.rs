//! R1CS circuit: prove that a WASM bytecode hash passed ruleset R with score ≥ S.
//!
//! # Public inputs (in allocation order — must match the verifier)
//! 1. `wasm_hash`        – SHA-256(wasm_bytes) mod |Fr|
//! 2. `ruleset_version`  – u32 identifying which rule set was applied
//! 3. `score_threshold`  – minimum passing score (integer in [0, N_RULES])
//! 4. `rules_commitment` – Poseidon hash of the per-rule result vector
//!
//! # Private witness
//! * `rule_results[i]` – per-rule boolean (true = pass, false = fail)
//!
//! # Constraints
//! 1. **Boolean**: `r_i * (1 − r_i) = 0` for each rule (enforced by `Boolean::new_witness`)
//! 2. **Score**: `score = Σ weight_i * r_i` where all weights = 1 for this PoC
//! 3. **Threshold**: bit-decomposition of `(score − threshold)` fits in `SCORE_BITS`
//!    bits, proving `score ≥ threshold` without revealing the exact score.
//! 4. **Commitment**: `Poseidon(r₀, …, r_{N−1}) == rules_commitment`

use ark_crypto_primitives::sponge::{
    constraints::CryptographicSpongeVar,
    poseidon::{constraints::PoseidonSpongeVar, PoseidonConfig},
};
use ark_ff::{BigInteger, PrimeField};
use ark_r1cs_std::{
    fields::fp::FpVar,
    prelude::{AllocVar, Boolean, EqGadget, FieldVar},
};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_std::{One, Zero};

use crate::{N_RULES, SCORE_BITS};

/// All public inputs for one audit proof.
#[derive(Clone, Debug)]
pub struct AuditPublicInputs<F: PrimeField> {
    /// SHA-256(wasm_bytes) reduced mod |Fr|
    pub wasm_hash: F,
    /// Which version of the ruleset was applied
    pub ruleset_version: F,
    /// Minimum score the contract must achieve
    pub score_threshold: F,
    /// Poseidon commitment to the per-rule result vector
    pub rules_commitment: F,
}

/// Private witness for one audit proof.
#[derive(Clone, Debug)]
pub struct AuditWitness {
    /// Per-rule pass (true) / fail (false) — the "finding set"
    pub rule_results: [bool; N_RULES],
}

/// The full Groth16 circuit.
///
/// Pass `witness: None` during the trusted setup (key generation) phase
/// and `witness: Some(...)` when generating a real proof.
#[derive(Clone)]
pub struct AuditCircuit<F: PrimeField> {
    pub public: AuditPublicInputs<F>,
    pub witness: Option<AuditWitness>,
    pub poseidon_params: PoseidonConfig<F>,
}

impl<F: PrimeField> ConstraintSynthesizer<F> for AuditCircuit<F> {
    fn generate_constraints(self, cs: ConstraintSystemRef<F>) -> Result<(), SynthesisError> {
        // ── 1. Public inputs ─────────────────────────────────────────────────
        let wasm_hash_var = FpVar::new_input(ark_relations::ns!(cs, "wasm_hash"), || {
            Ok(self.public.wasm_hash)
        })?;
        let _ruleset_ver_var = FpVar::new_input(ark_relations::ns!(cs, "ruleset_version"), || {
            Ok(self.public.ruleset_version)
        })?;
        let score_threshold_var =
            FpVar::new_input(ark_relations::ns!(cs, "score_threshold"), || {
                Ok(self.public.score_threshold)
            })?;
        let rules_commitment_var =
            FpVar::new_input(ark_relations::ns!(cs, "rules_commitment"), || {
                Ok(self.public.rules_commitment)
            })?;

        // wasm_hash is a binding public input — its value is already constrained
        // by being declared as `new_input`. No further constraint needed here;
        // it is included so the verifier can tie a proof to a specific WASM file.
        let _ = wasm_hash_var;

        // ── 2. Private witness: per-rule boolean results ──────────────────────
        let rule_result_bits: Vec<Boolean<F>> = (0..N_RULES)
            .map(|i| {
                Boolean::new_witness(ark_relations::ns!(cs, "rule"), || {
                    self.witness
                        .as_ref()
                        .map(|w| w.rule_results[i])
                        .ok_or(SynthesisError::AssignmentMissing)
                })
            })
            .collect::<Result<_, _>>()?;

        // ── 3. Compute score = Σ weight_i · r_i  (all weights = 1 in v1) ────
        // We convert each Boolean to an FpVar via conditional select, then sum.
        let mut score_var = FpVar::zero();
        let one = FpVar::constant(F::one());
        let zero = FpVar::zero();
        for bit in &rule_result_bits {
            let contribution = bit.select(&one, &zero)?;
            score_var += contribution;
        }

        // ── 4. Range check: score − threshold ≥ 0 (fits in SCORE_BITS bits) ──
        //
        // Strategy: witness each bit of diff = score − threshold, reconstruct
        // diff from those bits (adding SCORE_BITS boolean constraints), then
        // enforce that the reconstruction equals score − threshold.
        //
        // If score < threshold the diff would be ≡ (p − gap) mod p — a huge
        // number that doesn't fit in SCORE_BITS bits — making the constraint
        // system unsatisfiable for any bit assignment the prover might try.
        let diff_val: Option<F> = self.witness.as_ref().map(|w| {
            let score: u64 = w.rule_results.iter().filter(|&&b| b).count() as u64;
            let thr: u64 = {
                let f = self.public.score_threshold;
                // Extract the small integer value from the field element
                let bits = f.into_bigint();
                bits.as_ref()[0] // low 64-bit limb — sufficient for our range
            };
            F::from(score.saturating_sub(thr))
        });

        let diff_bit_vars: Vec<Boolean<F>> = (0..SCORE_BITS)
            .map(|i| {
                Boolean::new_witness(ark_relations::ns!(cs, "diff_bit"), || {
                    diff_val
                        .map(|d| {
                            // Extract bit i from the field element's integer representation
                            let limb = d.into_bigint().as_ref()[0];
                            (limb >> i) & 1 == 1
                        })
                        .ok_or(SynthesisError::AssignmentMissing)
                })
            })
            .collect::<Result<_, _>>()?;

        // Reconstruct diff from bits and enforce equality with score − threshold
        let mut diff_reconstructed = FpVar::zero();
        for (i, b) in diff_bit_vars.iter().enumerate() {
            let pow2 = FpVar::constant(F::from(1u64 << i));
            let term = b.select(&pow2, &zero)?;
            diff_reconstructed += term;
        }
        diff_reconstructed.enforce_equal(&(score_var.clone() - score_threshold_var.clone()))?;

        // ── 5. Commitment check: Poseidon(rule_results) == rules_commitment ──
        let fp_results: Vec<FpVar<F>> = rule_result_bits
            .iter()
            .map(|b| b.select(&one, &zero))
            .collect::<Result<_, _>>()?;

        let mut sponge = PoseidonSpongeVar::<F>::new(cs.clone(), &self.poseidon_params);
        sponge.absorb(&fp_results)?;
        let hash_out = sponge.squeeze_field_elements(1)?;
        hash_out[0].enforce_equal(&rules_commitment_var)?;

        Ok(())
    }
}
