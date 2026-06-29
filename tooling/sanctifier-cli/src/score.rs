//! Security scoring for `sanctifier attest` (#354).
//!
//! Runs the core analyzer over a contract/workspace, buckets findings by
//! severity, and folds them into a single 0..=100 security score. A clean scan
//! (no findings) scores 100. The exact source bytes are also hashed into a
//! commitment so an attestation is bound to precisely the code that was scanned.

use anyhow::{Context, Result};
use sanctifier_core::{Analyzer, SanctifyConfig, SizeWarningLevel};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::vulndb::VulnDatabase;

/// Per-severity penalty applied to the base score of 100.
const W_CRITICAL: u32 = 40;
const W_HIGH: u32 = 15;
const W_MEDIUM: u32 = 6;
const W_LOW: u32 = 2;

/// Outcome of scoring a scan.
pub struct ScanScore {
    pub score: u8,
    pub total_findings: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    /// SHA-256 (hex) over the analyzed source — binds the attestation to the code.
    pub source_commitment: String,
    /// Identifier of the ruleset used (built-in vuln DB version).
    pub ruleset: String,
    pub files_analyzed: usize,
}

#[derive(Default)]
struct Tally {
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
}

impl Tally {
    fn total(&self) -> usize {
        self.critical + self.high + self.medium + self.low
    }

    fn score(&self) -> u8 {
        let penalty = W_CRITICAL * self.critical as u32
            + W_HIGH * self.high as u32
            + W_MEDIUM * self.medium as u32
            + W_LOW * self.low as u32;
        100u32.saturating_sub(penalty).min(100) as u8
    }
}

/// Analyze `path` and compute its security score and source commitment.
pub fn score_path(path: &Path) -> Result<ScanScore> {
    let config = load_config(path);
    let analyzer = Analyzer::new(config.clone());
    let vuln_db = VulnDatabase::load_default();

    let mut files: Vec<(String, String)> = Vec::new();
    if path.is_dir() {
        collect_sources(path, &config, &mut files);
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        files.push((path.display().to_string(), content));
    } else {
        anyhow::bail!("{} is not a .rs file or directory", path.display());
    }

    // Deterministic source commitment: hash each (path, content) in path order.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hasher = Sha256::new();
    let mut tally = Tally::default();

    for (name, content) in &files {
        hasher.update(name.as_bytes());
        hasher.update([0u8]);
        hasher.update(content.as_bytes());
        hasher.update([0u8]);

        tally.critical += analyzer.scan_auth_gaps(content).len();
        for panic in analyzer.scan_panics(content) {
            if panic.issue_type == "panic!" {
                tally.critical += 1;
            } else {
                tally.high += 1;
            }
        }
        tally.high += analyzer.scan_arithmetic_overflow(content).len();
        tally.high += analyzer.scan_unhandled_results(content).len();
        tally.high += analyzer.verify_smt_invariants(content).len();
        tally.high += vuln_db.scan(content, name).len();
        for warning in analyzer.analyze_ledger_size(content) {
            if warning.level == SizeWarningLevel::ExceedsLimit {
                tally.high += 1;
            } else {
                tally.low += 1;
            }
        }
        tally.medium += analyzer.analyze_unsafe_patterns(content).len();
        tally.medium += analyzer.scan_events(content).len();
        tally.medium += analyzer.scan_storage_collisions(content).len();
        tally.medium += analyzer
            .analyze_custom_rules(content, &config.custom_rules)
            .len();
        tally.medium += analyzer.analyze_upgrade_patterns(content).findings.len();
    }

    let source_commitment = format!("{:x}", hasher.finalize());

    Ok(ScanScore {
        score: tally.score(),
        total_findings: tally.total(),
        critical: tally.critical,
        high: tally.high,
        medium: tally.medium,
        low: tally.low,
        source_commitment,
        ruleset: format!("builtin-vulndb@{}", vuln_db.version),
        files_analyzed: files.len(),
    })
}

fn collect_sources(dir: &Path, config: &SanctifyConfig, out: &mut Vec<(String, String)>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if config.ignore_paths.iter().any(|p| name.contains(p)) {
                continue;
            }
            collect_sources(&path, config, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                out.push((path.display().to_string(), content));
            }
        }
    }
}

fn load_config(path: &Path) -> SanctifyConfig {
    let mut current = if path.is_file() {
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    } else {
        path.to_path_buf()
    };

    loop {
        let config_path = current.join(".sanctify.toml");
        if config_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        if !current.pop() {
            break;
        }
    }
    SanctifyConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_contract(dir: &Path, name: &str, body: &str) {
        let path = dir.join(name);
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn clean_contract_scores_100() {
        let tmp = tempfile::tempdir().unwrap();
        write_contract(
            tmp.path(),
            "lib.rs",
            "pub fn add(a: u32, b: u32) -> u32 { a.saturating_add(b) }\n",
        );
        let result = score_path(tmp.path()).unwrap();
        assert_eq!(result.score, 100);
        assert_eq!(result.total_findings, 0);
        assert_eq!(result.files_analyzed, 1);
        assert_eq!(result.source_commitment.len(), 64);
    }

    #[test]
    fn findings_reduce_the_score() {
        let tmp = tempfile::tempdir().unwrap();
        // `panic!` is a critical finding → score must drop below 100.
        write_contract(
            tmp.path(),
            "lib.rs",
            "pub fn boom() { panic!(\"explode\"); }\n",
        );
        let result = score_path(tmp.path()).unwrap();
        assert!(result.score < 100, "expected penalty, got {}", result.score);
        assert!(result.total_findings > 0);
    }

    #[test]
    fn source_commitment_is_deterministic() {
        let tmp = tempfile::tempdir().unwrap();
        write_contract(tmp.path(), "lib.rs", "pub fn f() {}\n");
        let a = score_path(tmp.path()).unwrap();
        let b = score_path(tmp.path()).unwrap();
        assert_eq!(a.source_commitment, b.source_commitment);
    }
}
