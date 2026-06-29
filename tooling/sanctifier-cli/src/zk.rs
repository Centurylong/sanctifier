//! Zero-knowledge attestation backend for `sanctifier attest` (#354).
//!
//! The statement we prove is deliberately small and sound: **"the (hidden)
//! security score is at least the public threshold"**, bound to the exact
//! scanner version, ruleset, and source under analysis.
//!
//! We use a Bulletproofs range proof (no trusted setup) over `delta = score -
//! threshold`:
//!
//!   * a Pedersen commitment hides `delta` (and therefore the exact score) —
//!     the verifier learns only that the bar was cleared, not by how much;
//!   * the range proof shows `delta ∈ [0, 2^N)`, i.e. `score >= threshold`
//!     (and `score < threshold + 2^N`, trivially true for a 0..=100 score);
//!   * the Fiat–Shamir transcript is seeded with a binding over
//!     `scanner_version || ruleset || source_commitment || threshold`, so a
//!     proof produced for one scan/version cannot be replayed for another.
//!
//! This intentionally does **not** re-execute the scanner in-circuit (no
//! practical attestation system does); it binds the attestation to a specific,
//! named scanner+ruleset that a verifier chooses to trust, and proves the
//! score predicate in zero knowledge over that commitment.

use anyhow::{anyhow, Context, Result};
use bulletproofs::{BulletproofGens, PedersenGens, RangeProof};
use curve25519_dalek::ristretto::CompressedRistretto;
use curve25519_dalek::scalar::Scalar;
use merlin::Transcript;
use rand::rngs::OsRng;

/// Bit-width of the range proof. `delta = score - threshold` is at most 100 for
/// a 0..=100 score, so 16 bits is comfortably sufficient and a valid
/// Bulletproofs gate size.
pub const RANGE_BITS: usize = 16;

const TRANSCRIPT_LABEL: &[u8] = b"sanctifier-attest-v1";

/// A generated proof that a passing scan cleared the threshold, plus the
/// commitment the verifier needs.
#[derive(Debug, Clone)]
pub struct PassingProof {
    /// Serialized Bulletproofs range proof.
    pub proof_bytes: Vec<u8>,
    /// Compressed Pedersen commitment to `score - threshold`.
    pub commitment_bytes: [u8; 32],
}

fn transcript_for(binding: &[u8]) -> Transcript {
    let mut transcript = Transcript::new(TRANSCRIPT_LABEL);
    transcript.append_message(b"statement-binding", binding);
    transcript
}

/// Prove, in zero knowledge, that `score >= threshold`, binding the proof to
/// `binding` (a hash over scanner/ruleset/source/threshold).
///
/// Returns an error (rather than a proof) when `score < threshold`, so callers
/// can fail cleanly on a non-passing scan.
pub fn prove_passing(score: u64, threshold: u64, binding: &[u8]) -> Result<PassingProof> {
    let delta = score
        .checked_sub(threshold)
        .ok_or_else(|| anyhow!("score {score} is below the threshold {threshold}"))?;

    if delta >= (1u64 << RANGE_BITS) {
        return Err(anyhow!(
            "score margin {delta} exceeds the provable range (2^{RANGE_BITS})"
        ));
    }

    let pc_gens = PedersenGens::default();
    let bp_gens = BulletproofGens::new(RANGE_BITS, 1);
    let blinding = Scalar::random(&mut OsRng);

    let mut transcript = transcript_for(binding);
    let (proof, committed) =
        RangeProof::prove_single(&bp_gens, &pc_gens, &mut transcript, delta, &blinding, RANGE_BITS)
            .map_err(|e| anyhow!("failed to generate range proof: {e:?}"))?;

    Ok(PassingProof {
        proof_bytes: proof.to_bytes(),
        commitment_bytes: committed.to_bytes(),
    })
}

/// Verify a passing-scan proof against the public commitment, threshold, and
/// statement binding. Returns `Ok(())` only if the proof is valid and bound to
/// exactly this statement.
pub fn verify_passing(
    proof_bytes: &[u8],
    commitment_bytes: &[u8],
    binding: &[u8],
) -> Result<()> {
    let proof = RangeProof::from_bytes(proof_bytes)
        .map_err(|e| anyhow!("malformed range proof: {e:?}"))?;

    let commitment = CompressedRistretto::from_slice(commitment_bytes)
        .context("commitment must be 32 bytes")?;

    let pc_gens = PedersenGens::default();
    let bp_gens = BulletproofGens::new(RANGE_BITS, 1);

    let mut transcript = transcript_for(binding);
    proof
        .verify_single(&bp_gens, &pc_gens, &mut transcript, &commitment, RANGE_BITS)
        .map_err(|e| anyhow!("proof verification failed: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proves_and_verifies_a_passing_score() {
        let binding = b"scanner=1.0|ruleset=v3|src=abcd|t=90";
        let proof = prove_passing(93, 90, binding).expect("prove");
        verify_passing(&proof.proof_bytes, &proof.commitment_bytes, binding).expect("verify");
    }

    #[test]
    fn equal_to_threshold_is_provable() {
        let binding = b"binding";
        let proof = prove_passing(90, 90, binding).expect("prove");
        verify_passing(&proof.proof_bytes, &proof.commitment_bytes, binding).expect("verify");
    }

    #[test]
    fn below_threshold_cannot_be_proven() {
        let err = prove_passing(89, 90, b"binding").unwrap_err();
        assert!(err.to_string().contains("below the threshold"));
    }

    #[test]
    fn proof_is_bound_to_the_statement() {
        let proof = prove_passing(95, 90, b"statement-A").expect("prove");
        // A verifier using a different binding (different scanner/source/etc.)
        // must reject — proofs are non-transferable across statements.
        let result = verify_passing(&proof.proof_bytes, &proof.commitment_bytes, b"statement-B");
        assert!(result.is_err());
    }

    #[test]
    fn tampered_commitment_is_rejected() {
        let binding = b"binding";
        let proof = prove_passing(95, 90, binding).expect("prove");
        let mut bad = proof.commitment_bytes;
        bad[0] ^= 0x01;
        assert!(verify_passing(&proof.proof_bytes, &bad, binding).is_err());
    }
}
