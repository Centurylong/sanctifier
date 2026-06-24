//! Poseidon sponge parameters for the audit-proof circuit.
//!
//! # Security note
//! The MDS matrix and round constants below are generated from a fixed,
//! deterministic seed via a PRNG.  **These are PoC parameters only.**
//! Production use requires a trusted-setup or MPC parameter ceremony that
//! ensures the MDS matrix is a true Maximum Distance Separable matrix and
//! the round constants are sampled with a verifiable hash-to-field procedure
//! (e.g. the Grain LFSR used by the Poseidon authors).

use ark_ff::{PrimeField, UniformRand as _};

use ark_crypto_primitives::sponge::poseidon::PoseidonConfig;

/// Seed used to generate the PoC Poseidon parameters.
/// Changing this value will break existing proofs / verifying keys.
pub const POSEIDON_SEED: u64 = 0x534E_4354_4946_4945; // "SANCTIFIE" in ASCII

/// Sponge parameters: rate=2, capacity=1 (state size = 3), α=5.
pub const POSEIDON_RATE: usize = 2;
pub const POSEIDON_CAPACITY: usize = 1;
pub const POSEIDON_ALPHA: u64 = 5;
pub const POSEIDON_FULL_ROUNDS: usize = 8;
pub const POSEIDON_PARTIAL_ROUNDS: usize = 57;

/// Build a [`PoseidonConfig`] for field `F` from the fixed PoC seed.
pub fn poseidon_config<F: PrimeField>() -> PoseidonConfig<F> {
    use ark_std::rand::{rngs::StdRng, SeedableRng};
    let mut rng = StdRng::seed_from_u64(POSEIDON_SEED);

    let state = POSEIDON_RATE + POSEIDON_CAPACITY; // 3
    let total_rounds = POSEIDON_FULL_ROUNDS + POSEIDON_PARTIAL_ROUNDS;

    // Random MDS matrix (3×3).
    // WARNING: not guaranteed to be MDS — PoC only; replace with a proper
    // Cauchy or Vandermonde-derived MDS for production.
    let mds: Vec<Vec<F>> = (0..state)
        .map(|_| (0..state).map(|_| F::rand(&mut rng)).collect())
        .collect();

    // Random round constants.
    let ark: Vec<Vec<F>> = (0..total_rounds)
        .map(|_| (0..state).map(|_| F::rand(&mut rng)).collect())
        .collect();

    PoseidonConfig::new(
        POSEIDON_FULL_ROUNDS,
        POSEIDON_PARTIAL_ROUNDS,
        POSEIDON_ALPHA,
        mds,
        ark,
        POSEIDON_RATE,
        POSEIDON_CAPACITY,
    )
}
