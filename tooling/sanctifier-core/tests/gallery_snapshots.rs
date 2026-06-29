//! Golden snapshot tests over the canonical vulnerable-contract gallery.
//!
//! The gallery (issue #388) is a corpus of ten minimal Soroban contracts, each
//! isolating exactly one bug class, paired with a fixed counterpart. Every
//! fixture is run through the full default `RuleRegistry` and its findings are
//! snapshotted, so the gallery is wired directly into the detector snapshot
//! suite: when a detector's behaviour changes (or a new detector lands for a
//! currently-uncovered class), the affected snapshot diffs and must be reviewed.
//!
//! Bug class -> finding-code mapping lives in
//! `tests/fixtures/gallery/README.md`. Classes whose detector is not yet
//! implemented produce an empty snapshot today; that empty snapshot is the
//! regression baseline the future detector will be measured against.
//!
//! Review workflow: `cargo insta test` then `cargo insta review`
//! (see `tooling/sanctifier-core/tests/README.md`).

use sanctifier_core::rules::RuleRegistry;

/// Run every default detector over a gallery fixture and snapshot the findings.
fn assert_gallery_snapshot(name: &str, source: &str) {
    let findings = RuleRegistry::with_default_rules().run_all(source);
    insta::assert_yaml_snapshot!(name, findings);
}

/// Declare a `#[test]` that snapshots one gallery fixture.
macro_rules! gallery_case {
    ($test:ident, $name:literal, $file:literal) => {
        #[test]
        fn $test() {
            assert_gallery_snapshot($name, include_str!(concat!("fixtures/gallery/", $file)));
        }
    };
}

// re-initialization (S001)
gallery_case!(
    reinit_vulnerable,
    "reinit_vulnerable",
    "reinit_vulnerable.rs"
);
gallery_case!(reinit_fixed, "reinit_fixed", "reinit_fixed.rs");

// unchecked upgrade authorization (S010 / S001)
gallery_case!(
    upgrade_auth_vulnerable,
    "upgrade_auth_vulnerable",
    "upgrade_auth_vulnerable.rs"
);
gallery_case!(
    upgrade_auth_fixed,
    "upgrade_auth_fixed",
    "upgrade_auth_fixed.rs"
);

// CEI / reentrancy (S006, planned)
gallery_case!(
    reentrancy_vulnerable,
    "reentrancy_vulnerable",
    "reentrancy_vulnerable.rs"
);
gallery_case!(reentrancy_fixed, "reentrancy_fixed", "reentrancy_fixed.rs");

// unbounded loop (S006, planned)
gallery_case!(
    unbounded_loop_vulnerable,
    "unbounded_loop_vulnerable",
    "unbounded_loop_vulnerable.rs"
);
gallery_case!(
    unbounded_loop_fixed,
    "unbounded_loop_fixed",
    "unbounded_loop_fixed.rs"
);

// missing TTL bump (S006, planned)
gallery_case!(
    missing_ttl_vulnerable,
    "missing_ttl_vulnerable",
    "missing_ttl_vulnerable.rs"
);
gallery_case!(
    missing_ttl_fixed,
    "missing_ttl_fixed",
    "missing_ttl_fixed.rs"
);

// weak randomness (S006, planned)
gallery_case!(
    weak_randomness_vulnerable,
    "weak_randomness_vulnerable",
    "weak_randomness_vulnerable.rs"
);
gallery_case!(
    weak_randomness_fixed,
    "weak_randomness_fixed",
    "weak_randomness_fixed.rs"
);

// integer overflow (S003)
gallery_case!(
    integer_overflow_vulnerable,
    "integer_overflow_vulnerable",
    "integer_overflow_vulnerable.rs"
);
gallery_case!(
    integer_overflow_fixed,
    "integer_overflow_fixed",
    "integer_overflow_fixed.rs"
);

// allowance race (S006, planned)
gallery_case!(
    allowance_race_vulnerable,
    "allowance_race_vulnerable",
    "allowance_race_vulnerable.rs"
);
gallery_case!(
    allowance_race_fixed,
    "allowance_race_fixed",
    "allowance_race_fixed.rs"
);

// oracle staleness (S006, planned)
gallery_case!(
    oracle_staleness_vulnerable,
    "oracle_staleness_vulnerable",
    "oracle_staleness_vulnerable.rs"
);
gallery_case!(
    oracle_staleness_fixed,
    "oracle_staleness_fixed",
    "oracle_staleness_fixed.rs"
);

// confused-deputy authorization (S001 family, planned refinement)
gallery_case!(
    confused_deputy_vulnerable,
    "confused_deputy_vulnerable",
    "confused_deputy_vulnerable.rs"
);
gallery_case!(
    confused_deputy_fixed,
    "confused_deputy_fixed",
    "confused_deputy_fixed.rs"
);
