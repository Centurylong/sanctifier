//! `sanctifier attest` — generate (or verify) a zero-knowledge attestation
//! that a scan passed a score threshold (#354).
//!
//! Generation runs the analyzer, computes a security score, and — only if the
//! score clears `--threshold` — emits a `{proof, public_inputs, meta}` artifact
//! containing a Bulletproofs proof that the (hidden) score met the bar, bound
//! to the exact scanner version, ruleset, and source. Verification recomputes
//! the statement binding from the artifact's own metadata before checking the
//! proof, so a tampered binding can't slip through.

use anyhow::{Context, Result};
use clap::Args;
use colored::*;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::score::score_path;
use crate::zk::{prove_passing, verify_passing, RANGE_BITS};

const ARTIFACT_VERSION: &str = "sanctifier-attestation-v1";
const BINDING_DOMAIN: &[u8] = b"sanctifier-attest-binding-v1";
const PROOF_SYSTEM: &str = "bulletproofs-rangeproof";

#[derive(Args, Debug)]
pub struct AttestArgs {
    /// Path to the contract directory or a single .rs file
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Minimum security score (0-100) the scan must reach to attest
    #[arg(short, long, default_value = "90")]
    pub threshold: u8,

    /// Write the attestation artifact here (defaults to stdout)
    #[arg(short, long)]
    pub out: Option<PathBuf>,

    /// Verify an existing attestation artifact instead of generating one
    #[arg(long, value_name = "FILE")]
    pub verify: Option<PathBuf>,
}

/// Bind the proof to exactly this scanner+ruleset+source+threshold so it can't
/// be replayed for a different statement.
fn compute_binding(
    scanner_version: &str,
    ruleset: &str,
    source_commitment: &str,
    threshold: u8,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update([0u8]);
    hasher.update(scanner_version.as_bytes());
    hasher.update([0u8]);
    hasher.update(ruleset.as_bytes());
    hasher.update([0u8]);
    hasher.update(source_commitment.as_bytes());
    hasher.update([0u8]);
    hasher.update(threshold.to_le_bytes());
    hasher.finalize().into()
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn exec(args: AttestArgs) -> Result<()> {
    if let Some(artifact) = args.verify.clone() {
        return verify_artifact(&artifact);
    }
    generate(args)
}

fn generate(args: AttestArgs) -> Result<()> {
    let scanner_version = env!("CARGO_PKG_VERSION");
    let scan = score_path(&args.path)?;

    eprintln!(
        "{} Scanned {} file(s) — {} finding(s) [critical: {}, high: {}, medium: {}, low: {}]; \
         attesting against threshold {}.",
        "🔍".blue(),
        scan.files_analyzed,
        scan.total_findings,
        scan.critical,
        scan.high,
        scan.medium,
        scan.low,
        args.threshold
    );

    // Fail cleanly when the scan does not clear the bar — no artifact is written.
    if scan.score < args.threshold {
        eprintln!(
            "{} Scan did not pass: score is below the required threshold of {}. \
             Fix findings and re-run; no attestation was produced.",
            "❌".red(),
            args.threshold
        );
        std::process::exit(1);
    }

    let binding = compute_binding(
        scanner_version,
        &scan.ruleset,
        &scan.source_commitment,
        args.threshold,
    );

    let proof = prove_passing(scan.score as u64, args.threshold as u64, &binding)
        .context("failed to generate the zero-knowledge proof")?;

    // Never emit a proof we can't verify ourselves.
    verify_passing(&proof.proof_bytes, &proof.commitment_bytes, &binding)
        .context("internal error: generated proof failed self-verification")?;

    // The exact score and per-severity counts are intentionally NOT published:
    // the proof reveals only that score >= threshold (zero-knowledge).
    let artifact = serde_json::json!({
        "version": ARTIFACT_VERSION,
        "proof": {
            "system": PROOF_SYSTEM,
            "range_bits": RANGE_BITS,
            "proof": hex::encode(&proof.proof_bytes),
        },
        "public_inputs": {
            "score_commitment": hex::encode(proof.commitment_bytes),
            "threshold": args.threshold,
            "statement_binding": hex::encode(binding),
        },
        "meta": {
            "scanner": "sanctifier",
            "scanner_version": scanner_version,
            "ruleset": scan.ruleset,
            "source_commitment": scan.source_commitment,
            "files_analyzed": scan.files_analyzed,
            "result": "pass",
            "claim": "security score >= threshold",
            "generated_at_unix": unix_timestamp(),
        },
        "verification": {
            "instructions":
                "Recompute the statement binding from meta (scanner_version, ruleset, \
                 source_commitment, threshold), then verify the Bulletproofs range proof \
                 against score_commitment. Locally: `sanctifier attest --verify <file>`.",
            "command": "sanctifier attest --verify attestation.json",
        },
    });

    let rendered = serde_json::to_string_pretty(&artifact)?;
    match &args.out {
        Some(path) => {
            std::fs::write(path, format!("{rendered}\n"))
                .with_context(|| format!("failed to write {}", path.display()))?;
            eprintln!(
                "{} Attestation written to {} (score \u{2265} {}, proven in zero knowledge).",
                "✅".green(),
                path.display(),
                args.threshold
            );
        }
        None => println!("{rendered}"),
    }

    Ok(())
}

fn verify_artifact(path: &PathBuf) -> Result<()> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let artifact: serde_json::Value =
        serde_json::from_str(&raw).context("artifact is not valid JSON")?;

    let proof_hex = artifact["proof"]["proof"]
        .as_str()
        .context("missing proof.proof")?;
    let commitment_hex = artifact["public_inputs"]["score_commitment"]
        .as_str()
        .context("missing public_inputs.score_commitment")?;
    let threshold = artifact["public_inputs"]["threshold"]
        .as_u64()
        .context("missing public_inputs.threshold")?;
    let claimed_binding = artifact["public_inputs"]["statement_binding"]
        .as_str()
        .context("missing public_inputs.statement_binding")?;

    let scanner_version = artifact["meta"]["scanner_version"]
        .as_str()
        .context("missing meta.scanner_version")?;
    let ruleset = artifact["meta"]["ruleset"]
        .as_str()
        .context("missing meta.ruleset")?;
    let source_commitment = artifact["meta"]["source_commitment"]
        .as_str()
        .context("missing meta.source_commitment")?;

    // Recompute the binding from the published metadata: this is what makes the
    // attestation sound. If the prover lied about scanner/ruleset/source or the
    // threshold, the recomputed binding won't match the proof and it is rejected.
    let binding = compute_binding(
        scanner_version,
        ruleset,
        source_commitment,
        u8::try_from(threshold).context("threshold out of range")?,
    );
    if hex::encode(binding) != claimed_binding {
        anyhow::bail!(
            "statement binding does not match the published metadata — artifact is inconsistent"
        );
    }

    let proof_bytes = hex::decode(proof_hex).context("proof is not valid hex")?;
    let commitment_bytes = hex::decode(commitment_hex).context("commitment is not valid hex")?;

    verify_passing(&proof_bytes, &commitment_bytes, &binding)?;

    println!(
        "{} Attestation valid: proven that the security score is \u{2265} {} for {} (ruleset {}).",
        "✅".green(),
        threshold,
        source_commitment,
        ruleset
    );
    Ok(())
}
