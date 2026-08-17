//! Real-world corpus benchmark (#764).
//!
//! `tests/fixtures/gallery` proves Sanctifier *catches* known-bad patterns;
//! this proves it behaves sanely on known-good, officially-maintained
//! production code — no panics/hangs, and a bounded, recorded finding
//! count so a detector regression that suddenly floods real code with
//! false positives is visible in the diff of `SCORECARD.md` rather than
//! discovered downstream.
//!
//! See `fixtures/corpus/real_world/MANIFEST.json` for provenance and the
//! per-contract label set, and `ATTRIBUTION.md` for the license notice
//! covering the vendored contracts.

use sanctifier_core::rules::RuleRegistry;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

const CORPUS: &[(&str, &str)] = &[
    (
        "atomic_swap",
        include_str!("fixtures/corpus/real_world/atomic_swap.rs"),
    ),
    (
        "increment",
        include_str!("fixtures/corpus/real_world/increment.rs"),
    ),
    (
        "liquidity_pool",
        include_str!("fixtures/corpus/real_world/liquidity_pool.rs"),
    ),
    ("pause", include_str!("fixtures/corpus/real_world/pause.rs")),
    (
        "single_offer",
        include_str!("fixtures/corpus/real_world/single_offer.rs"),
    ),
    (
        "timelock",
        include_str!("fixtures/corpus/real_world/timelock.rs"),
    ),
    ("ttl", include_str!("fixtures/corpus/real_world/ttl.rs")),
    (
        "account",
        include_str!("fixtures/corpus/real_world/account.rs"),
    ),
];

/// Ceiling on *total* findings across the whole corpus, set from the actual
/// recomputed count (121, see SCORECARD.md) with headroom for legitimate
/// growth as the corpus expands toward the 20+ target. This isn't meant to
/// assert "zero false positives" (real code can legitimately trip a
/// detector, e.g. `ttl.rs` exists specifically to demonstrate
/// `extend_ttl`, which `missing_ttl` still flags pre-extension) — it's a
/// tripwire for a regression that starts flagging every line of ordinary
/// Soroban code.
const MAX_TOTAL_FINDINGS: usize = 180;

#[test]
fn real_world_corpus_does_not_panic_or_hang() {
    let registry = RuleRegistry::with_default_rules();

    for (name, source) in CORPUS {
        let start = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| registry.run_all(source)));
        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "RuleRegistry::run_all panicked on real-world contract '{name}'"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "RuleRegistry::run_all took {elapsed:?} on real-world contract '{name}' (possible hang)"
        );
    }
}

#[test]
fn real_world_corpus_finding_volume_is_bounded() {
    let registry = RuleRegistry::with_default_rules();

    let total: usize = CORPUS
        .iter()
        .map(|(_, source)| registry.run_all(source).len())
        .sum();

    assert!(
        total <= MAX_TOTAL_FINDINGS,
        "real-world corpus produced {total} findings across {} contracts, exceeding the {MAX_TOTAL_FINDINGS} ceiling — \
         check for a detector regression flooding legitimate code with false positives (see SCORECARD.md)",
        CORPUS.len()
    );
}
