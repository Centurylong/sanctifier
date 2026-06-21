//! Differential-testing harness vs Slither/Aderyn on overlapping checks (issue #503).
//!
//! Sanctifier targets Stellar Soroban (Rust/WASM); Slither and Aderyn target
//! Solidity/EVM, so a line-for-line cross-run is impossible. Instead we compare
//! *where checks overlap* — at the vulnerability-class level — using a shared
//! corpus (`tests/fixtures/corpus/differential-corpus.json`) that maps each
//! canonical bug class to:
//!   * the Soroban gallery fixtures (vulnerable + fixed),
//!   * the finding codes Sanctifier emits today (ground truth), and
//!   * the closest default Slither / Aderyn detectors for the same class.
//!
//! This file is the Sanctifier half of the harness: it runs the default
//! `RuleRegistry` over every corpus fixture, asserts the recorded ground truth
//! still holds, and prints the cross-analyzer comparison matrix. The EVM half
//! (running Slither/Aderyn live over the Solidity mirrors) is driven by
//! `scripts/differential-test.sh`, which degrades gracefully when those tools
//! are not installed.
//!
//! Run with output:
//!   cargo test -p sanctifier-core --test differential_test -- --nocapture
//! (locally without Z3, add `--no-default-features`).

use sanctifier_core::rules::RuleRegistry;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const MANIFEST_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/corpus/differential-corpus.json"
));

const VALID_OVERLAPS: &[&str] = &[
    "shared-covered",
    "shared-sanctifier-gap",
    "divergent-approach",
    "soroban-specific",
    "mutual-gap",
];

#[derive(Debug, Deserialize)]
struct Corpus {
    sanctifier_rule_to_code: BTreeMap<String, String>,
    corpus: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    bug_class: String,
    soroban: Fixtures,
    sanctifier: SanctifierExpectation,
    slither: ToolExpectation,
    aderyn: ToolExpectation,
    overlap: String,
}

#[derive(Debug, Deserialize)]
struct Fixtures {
    vulnerable: String,
    fixed: String,
}

#[derive(Debug, Deserialize)]
struct SanctifierExpectation {
    observed_codes: Vec<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ToolExpectation {
    detectors: Vec<String>,
    expected: bool,
}

fn load_corpus() -> Corpus {
    serde_json::from_str(MANIFEST_JSON).expect("differential-corpus.json must be valid JSON")
}

fn gallery_path(file: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/fixtures/gallery");
    p.push(file);
    p
}

fn read_fixture(file: &str) -> String {
    let path = gallery_path(file);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read fixture {path:?}: {e}"))
}

/// Run the default detector set over a source and return the set of finding
/// codes it reports (rule names mapped through the corpus rule→code table).
fn sanctifier_codes(source: &str, rule_to_code: &BTreeMap<String, String>) -> BTreeSet<String> {
    RuleRegistry::with_default_rules()
        .run_all(source)
        .into_iter()
        .map(|v| {
            rule_to_code
                .get(&v.rule_name)
                .unwrap_or_else(|| {
                    panic!(
                        "rule '{}' has no entry in sanctifier_rule_to_code; \
                         add it to differential-corpus.json",
                        v.rule_name
                    )
                })
                .clone()
        })
        .collect()
}

/// The Sanctifier ground truth recorded in the corpus must match what the
/// default detectors actually emit on each vulnerable fixture. This keeps the
/// differential corpus honest and fails loudly if a detector's coverage shifts.
#[test]
fn sanctifier_coverage_matches_corpus() {
    let corpus = load_corpus();
    for entry in &corpus.corpus {
        let source = read_fixture(&entry.soroban.vulnerable);
        let observed = sanctifier_codes(&source, &corpus.sanctifier_rule_to_code);
        let expected: BTreeSet<String> = entry.sanctifier.observed_codes.iter().cloned().collect();
        assert_eq!(
            observed, expected,
            "[{}] Sanctifier output drifted from the corpus on {}: expected {:?}, got {:?}",
            entry.bug_class, entry.soroban.vulnerable, expected, observed
        );
    }
}

/// Every `*_fixed` fixture is the corrected contract and must produce no
/// findings — this is the false-positive guard for the corpus.
#[test]
fn fixed_fixtures_are_clean() {
    let corpus = load_corpus();
    for entry in &corpus.corpus {
        let source = read_fixture(&entry.soroban.fixed);
        let observed = sanctifier_codes(&source, &corpus.sanctifier_rule_to_code);
        assert!(
            observed.is_empty(),
            "[{}] fixed fixture {} should be clean but reported {:?}",
            entry.bug_class,
            entry.soroban.fixed,
            observed
        );
    }
}

/// The rule→code table must cover every rule the default registry can emit,
/// otherwise `sanctifier_codes` would panic on an unmapped rule in the field.
#[test]
fn rule_to_code_map_covers_all_default_rules() {
    let corpus = load_corpus();
    for rule in RuleRegistry::with_default_rules().available_rules() {
        assert!(
            corpus.sanctifier_rule_to_code.contains_key(rule),
            "default rule '{rule}' is missing from sanctifier_rule_to_code in differential-corpus.json"
        );
    }
}

/// Manifest self-consistency: overlap labels are from the legend, a class is
/// "planned" iff Sanctifier emits nothing for it today, and any tool marked
/// `expected` must name at least one detector.
#[test]
fn corpus_is_internally_consistent() {
    let corpus = load_corpus();
    for entry in &corpus.corpus {
        assert!(
            VALID_OVERLAPS.contains(&entry.overlap.as_str()),
            "[{}] unknown overlap label '{}'",
            entry.bug_class,
            entry.overlap
        );

        let planned = entry.sanctifier.status == "planned";
        let empty = entry.sanctifier.observed_codes.is_empty();
        assert_eq!(
            planned, empty,
            "[{}] status='{}' but observed_codes={:?}: a class is 'planned' iff Sanctifier emits nothing",
            entry.bug_class, entry.sanctifier.status, entry.sanctifier.observed_codes
        );

        if entry.slither.expected {
            assert!(
                !entry.slither.detectors.is_empty(),
                "[{}] slither.expected=true but no detectors listed",
                entry.bug_class
            );
        }
        if entry.aderyn.expected {
            assert!(
                !entry.aderyn.detectors.is_empty(),
                "[{}] aderyn.expected=true but no detectors listed",
                entry.bug_class
            );
        }
    }
}

/// Not an assertion — prints the cross-analyzer comparison matrix and a summary
/// so the harness output doubles as the report body. See it with `--nocapture`.
#[test]
fn print_differential_report() {
    let corpus = load_corpus();
    println!("\n=== Sanctifier vs Slither/Aderyn — differential coverage (issue #503) ===\n");
    println!(
        "{:<18} {:<14} {:<10} {:<8} {:<8} overlap",
        "bug_class", "sanctifier", "slither", "aderyn", "status"
    );
    println!("{}", "-".repeat(86));

    let (mut sanct, mut slith, mut ader) = (0usize, 0usize, 0usize);
    for entry in &corpus.corpus {
        let source = read_fixture(&entry.soroban.vulnerable);
        let codes = sanctifier_codes(&source, &corpus.sanctifier_rule_to_code);
        let sanct_hit = !codes.is_empty();
        if sanct_hit {
            sanct += 1;
        }
        if entry.slither.expected {
            slith += 1;
        }
        if entry.aderyn.expected {
            ader += 1;
        }
        let codes_str = if codes.is_empty() {
            "—".to_string()
        } else {
            codes.iter().cloned().collect::<Vec<_>>().join(",")
        };
        println!(
            "{:<18} {:<14} {:<10} {:<8} {:<8} {}",
            entry.bug_class,
            codes_str,
            if entry.slither.expected { "yes" } else { "—" },
            if entry.aderyn.expected { "yes" } else { "—" },
            entry.sanctifier.status,
            entry.overlap,
        );
    }

    let n = corpus.corpus.len();
    println!("{}", "-".repeat(86));
    println!(
        "coverage on {n} overlapping classes: Sanctifier {sanct}/{n}, Slither {slith}/{n}, Aderyn {ader}/{n}"
    );
    let gaps: Vec<&str> = corpus
        .corpus
        .iter()
        .filter(|e| e.overlap == "shared-sanctifier-gap")
        .map(|e| e.bug_class.as_str())
        .collect();
    println!("Sanctifier gaps where an EVM tool already detects the class: {gaps:?}");
    println!("(full write-up: docs/differential-testing.md)\n");

    // sanity: the printed table covers the whole corpus
    assert_eq!(n, 10, "expected the canonical 10-class gallery corpus");
}
