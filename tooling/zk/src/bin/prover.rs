//! `sanctifier-prover` — CLI tool for generating and verifying audit proofs.
//!
//! Usage:
//!   sanctifier-prover <wasm-file> --threshold <N> [--ruleset-version <V>]
//!
//! The tool:
//!  1. SHA-256-hashes the WASM file.
//!  2. Simulates a mock audit that marks the first `<threshold>` rules as
//!     passing (sufficient to meet the threshold).
//!  3. Runs a trusted setup (groth16 key generation).
//!  4. Generates a proof and verifies it.
//!  5. Prints timing and proof-size metrics as JSON.

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result};
use ark_std::rand::SeedableRng;
use serde::Serialize;

use sanctifier_zk::{encoding, params, prove, setup, verify, N_RULES};

#[derive(Serialize)]
struct BenchResult {
    wasm_file: String,
    wasm_bytes: usize,
    ruleset_version: u32,
    score_threshold: u64,
    score_achieved: u64,
    constraint_count: usize,
    setup_ms: u128,
    prove_ms: u128,
    verify_ms: u128,
    proof_size_bytes: usize,
    verified: bool,
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Minimal arg parsing
    let wasm_path = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/dev/null"));

    let threshold: u64 = args
        .windows(2)
        .find(|w| w[0] == "--threshold")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(9);

    let ruleset_version: u32 = args
        .windows(2)
        .find(|w| w[0] == "--ruleset-version")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(1);

    // Read WASM (fall back to dummy bytes if /dev/null or missing)
    let wasm_bytes: Vec<u8> = std::fs::read(&wasm_path)
        .unwrap_or_else(|_| b"\x00asm\x01\x00\x00\x00".to_vec()); // minimal WASM magic

    eprintln!(
        "[sanctifier-prover] auditing {} ({} bytes), threshold={}, ruleset=v{}",
        wasm_path.display(),
        wasm_bytes.len(),
        threshold,
        ruleset_version,
    );

    // Build mock rule results: first `threshold` rules pass, rest fail
    let score_achieved = threshold.min(N_RULES as u64);
    let rule_results: [bool; N_RULES] = std::array::from_fn(|i| (i as u64) < score_achieved);

    // Build public statement
    let stmt = sanctifier_zk::AuditStatement {
        wasm_hash: encoding::wasm_hash_field(&wasm_bytes),
        ruleset_version: encoding::ruleset_version_field(ruleset_version),
        score_threshold: encoding::score_threshold_field(threshold),
        rules_commitment: encoding::rules_commitment(&rule_results),
    };

    // Trusted setup
    eprintln!("[sanctifier-prover] running trusted setup …");
    // Seed from system time for non-deterministic proofs
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xDEAD_BEEF_CAFE_BABE);
    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(seed);

    let t_setup = Instant::now();
    let (pk, vk) = setup(&mut rng).context("setup failed")?;
    let setup_ms = t_setup.elapsed().as_millis();
    eprintln!("[sanctifier-prover] setup done in {setup_ms} ms");

    // Prove
    eprintln!("[sanctifier-prover] generating proof …");
    let t_prove = Instant::now();
    let proof = prove(&stmt, &rule_results, &pk, &mut rng).context("prove failed")?;
    let prove_ms = t_prove.elapsed().as_millis();
    eprintln!("[sanctifier-prover] proof generated in {prove_ms} ms");

    let proof_bytes = sanctifier_zk::proof_to_bytes(&proof);

    // Verify
    let t_verify = Instant::now();
    let verified = verify(&stmt, &proof, &vk).context("verify failed")?;
    let verify_ms = t_verify.elapsed().as_millis();
    eprintln!("[sanctifier-prover] verify done in {verify_ms} ms → {verified}");

    let constraint_count = sanctifier_zk::constraint_count();

    let result = BenchResult {
        wasm_file: wasm_path.display().to_string(),
        wasm_bytes: wasm_bytes.len(),
        ruleset_version,
        score_threshold: threshold,
        score_achieved,
        constraint_count,
        setup_ms,
        prove_ms,
        verify_ms,
        proof_size_bytes: proof_bytes.len(),
        verified,
    };

    println!("{}", serde_json::to_string_pretty(&result)?);

    if !verified {
        std::process::exit(1);
    }

    Ok(())
}
