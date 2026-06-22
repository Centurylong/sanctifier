//! Golden snapshot tests for every detector.
//!
//! Each detector gets a dedicated fixture under `tests/fixtures/detectors/` and
//! a reviewed `insta` snapshot of the `RuleViolation`s it produces. CI runs these
//! as part of the normal test suite, so any unintended change to a detector's
//! output fails the build until the snapshot is re-reviewed.
//!
//! Workflow:
//!   * `cargo insta test`   — run the snapshot tests.
//!   * `cargo insta review` — interactively accept/reject pending changes.
//!   * `cargo insta accept` — accept all pending changes (use with care).
//!
//! See `tooling/sanctifier-core/tests/README.md` for the full guide.

use sanctifier_core::rules::{
    arithmetic_overflow::ArithmeticOverflowRule, auth_gap::AuthGapRule,
    edge_amount::EdgeAmountRule, error_code_collision::ErrorCodeCollisionRule,
    hardcoded_addr::HardcodedAddrRule, ledger_size::LedgerSizeRule,
    panic_detection::PanicDetectionRule, unhandled_result::UnhandledResultRule,
    unused_variable::UnusedVariableRule, Rule,
};

/// Run a detector against its fixture and snapshot the resulting findings.
///
/// The snapshot name is set explicitly so each detector maps to a stable,
/// human-readable snapshot file (`snapshots/detector_snapshots__<name>.snap`).
fn assert_detector_snapshot(name: &str, rule: &dyn Rule, fixture: &str) {
    let findings = rule.check(fixture);
    insta::assert_yaml_snapshot!(name, findings);
}

#[test]
fn snapshot_auth_gap() {
    assert_detector_snapshot(
        "auth_gap",
        &AuthGapRule::new(),
        include_str!("fixtures/detectors/auth_gap.rs"),
    );
}

#[test]
fn snapshot_arithmetic_overflow() {
    assert_detector_snapshot(
        "arithmetic_overflow",
        &ArithmeticOverflowRule::new(),
        include_str!("fixtures/detectors/arithmetic_overflow.rs"),
    );
}

#[test]
fn snapshot_unhandled_result() {
    assert_detector_snapshot(
        "unhandled_result",
        &UnhandledResultRule::new(),
        include_str!("fixtures/detectors/unhandled_result.rs"),
    );
}

#[test]
fn snapshot_unused_variable() {
    assert_detector_snapshot(
        "unused_variable",
        &UnusedVariableRule::new(),
        include_str!("fixtures/detectors/unused_variable.rs"),
    );
}

#[test]
fn snapshot_panic_detection() {
    assert_detector_snapshot(
        "panic_detection",
        &PanicDetectionRule::new(),
        include_str!("fixtures/detectors/panic_detection.rs"),
    );
}

#[test]
fn snapshot_ledger_size() {
    assert_detector_snapshot(
        "ledger_size",
        &LedgerSizeRule::new(),
        include_str!("fixtures/detectors/ledger_size.rs"),
    );
}

#[test]
fn snapshot_hardcoded_addr() {
    assert_detector_snapshot(
        "hardcoded_addr",
        &HardcodedAddrRule::new(),
        include_str!("fixtures/detectors/hardcoded_addr.rs"),
    );
}

#[test]
fn snapshot_error_code_collision() {
    assert_detector_snapshot(
        "error_code_collision",
        &ErrorCodeCollisionRule::new(),
        include_str!("fixtures/detectors/error_code_collision.rs"),
    );
}

#[test]
fn snapshot_edge_amount() {
    assert_detector_snapshot(
        "edge_amount",
        &EdgeAmountRule::new(),
        include_str!("fixtures/detectors/edge_amount.rs"),
    );
}
