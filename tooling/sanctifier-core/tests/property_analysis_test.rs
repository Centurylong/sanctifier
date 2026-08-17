//! Property tests: analysis is idempotent and order-independent (#766).
//!
//! Determinism is foundational for baselines and snapshots — a finding that
//! silently appears/disappears depending on run order or repeat invocation
//! would corrupt `.sanctify-baseline.json` suppression and make CI flaky.
//! These tests assert two invariants directly against `RuleRegistry`, the
//! primitive every caller (CLI directory walker, `analyze` command, baseline
//! diffing) builds on:
//!
//!   * **Idempotence** — running the full rule suite twice on the same
//!     source yields byte-for-byte identical findings.
//!   * **Order-independence** — analyzing a set of file contents in any
//!     order yields the same aggregate *set* of findings; no rule's output
//!     depends on what ran before it (no shared/mutable state leaking
//!     between `check` calls).

use sanctifier_core::rules::{RuleRegistry, RuleViolation};
use std::collections::HashSet;

const FIXTURES: &[&str] = &[
    include_str!("fixtures/detectors/auth_gap.rs"),
    include_str!("fixtures/detectors/sanct_visibility.rs"),
    include_str!("fixtures/detectors/arithmetic_overflow.rs"),
    include_str!("fixtures/detectors/panic_detection.rs"),
    include_str!("fixtures/detectors/unbounded_storage.rs"),
    include_str!("fixtures/detectors/shift_overflow.rs"),
    include_str!("fixtures/detectors/state_write_in_view.rs"),
    include_str!("fixtures/detectors/tier_boundary_off_by_one.rs"),
];

/// Reduce a violation to the fields that identify *what* was found, so sets
/// built from differently-ordered runs can be compared for equality without
/// caring about `Vec` position.
fn fingerprint(v: &RuleViolation) -> (String, String, String) {
    (v.rule_name.clone(), v.message.clone(), v.location.clone())
}

fn fingerprints(violations: &[RuleViolation]) -> Vec<(String, String, String)> {
    violations.iter().map(fingerprint).collect()
}

#[test]
fn analysis_is_idempotent_per_source() {
    let registry = RuleRegistry::with_default_rules();

    for (i, fixture) in FIXTURES.iter().enumerate() {
        let first = fingerprints(&registry.run_all(fixture));
        let second = fingerprints(&registry.run_all(fixture));
        assert_eq!(
            first, second,
            "fixture #{i}: re-running the rule suite on identical source produced different findings"
        );
    }
}

#[test]
fn analysis_is_idempotent_across_repeated_runs() {
    let registry = RuleRegistry::with_default_rules();
    let fixture = FIXTURES[0];

    let baseline = fingerprints(&registry.run_all(fixture));
    for run in 0..5 {
        let repeat = fingerprints(&registry.run_all(fixture));
        assert_eq!(
            baseline, repeat,
            "run {run}: findings drifted after repeated invocation"
        );
    }
}

#[test]
fn analysis_is_order_independent_across_files() {
    let registry = RuleRegistry::with_default_rules();

    let forward: HashSet<_> = FIXTURES
        .iter()
        .flat_map(|src| fingerprints(&registry.run_all(src)))
        .collect();

    let reversed: HashSet<_> = FIXTURES
        .iter()
        .rev()
        .flat_map(|src| fingerprints(&registry.run_all(src)))
        .collect();

    assert_eq!(
        forward, reversed,
        "aggregate findings differ depending on the order files were analyzed in"
    );

    // A handful of arbitrary permutations beyond just forward/reverse, to
    // catch order-dependence that a simple reversal could miss (e.g. state
    // that only leaks after exactly N prior calls).
    let permutations: [&[usize]; 3] = [
        &[3, 0, 5, 1, 7, 2, 4, 6],
        &[7, 6, 5, 4, 3, 2, 1, 0],
        &[1, 3, 5, 7, 0, 2, 4, 6],
    ];
    for order in permutations {
        let permuted: HashSet<_> = order
            .iter()
            .flat_map(|&idx| fingerprints(&registry.run_all(FIXTURES[idx])))
            .collect();
        assert_eq!(
            forward, permuted,
            "permuted order {order:?} changed the aggregate finding set"
        );
    }
}

#[test]
fn single_file_order_independence_is_consistent_with_isolated_run() {
    // Each file's findings must match what it produces in isolation,
    // regardless of which other files were analyzed before it in the batch.
    let registry = RuleRegistry::with_default_rules();

    let isolated: Vec<Vec<(String, String, String)>> = FIXTURES
        .iter()
        .map(|src| fingerprints(&registry.run_all(src)))
        .collect();

    for (i, src) in FIXTURES.iter().enumerate().rev() {
        let in_batch_context = fingerprints(&registry.run_all(src));
        assert_eq!(
            isolated[i], in_batch_context,
            "fixture #{i}: findings changed depending on batch processing order"
        );
    }
}
