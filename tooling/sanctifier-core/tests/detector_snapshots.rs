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
    arg_dos::ArgDosRule, arithmetic_overflow::ArithmeticOverflowRule, auth_gap::AuthGapRule,
    edge_amount::EdgeAmountRule, error_code_collision::ErrorCodeCollisionRule,
    fee_rounding::FeeRoundingRule, hardcoded_addr::HardcodedAddrRule,
    init_hardcoded_admin::InitHardcodedAdminRule, ledger_size::LedgerSizeRule,
    missing_ttl::MissingTtlRule, panic_detection::PanicDetectionRule,
    sanct_unwrap::SanctUnwrapRule, unhandled_result::UnhandledResultRule,
    unused_variable::UnusedVariableRule, Rule, RuleRegistry,
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

#[test]
fn snapshot_fee_rounding() {
    assert_detector_snapshot(
        "fee_rounding",
        &FeeRoundingRule::new(),
        include_str!("fixtures/detectors/fee_rounding.rs"),
    );
}

#[test]
fn snapshot_missing_ttl() {
    assert_detector_snapshot(
        "missing_ttl",
        &MissingTtlRule::new(),
        include_str!("fixtures/detectors/missing_ttl.rs"),
    );
}

#[test]
fn snapshot_arg_dos() {
    assert_detector_snapshot(
        "arg_dos",
        &ArgDosRule::new(),
        include_str!("fixtures/detectors/arg_dos.rs"),
    );
}

#[test]
fn snapshot_sanct_unwrap() {
    assert_detector_snapshot(
        "sanct_unwrap",
        &SanctUnwrapRule::new(),
        include_str!("fixtures/detectors/sanct_unwrap.rs"),
    );
}

#[test]
fn snapshot_init_hardcoded_admin() {
    assert_detector_snapshot(
        "init_hardcoded_admin",
        &InitHardcodedAdminRule::new(),
        include_str!("fixtures/detectors/init_hardcoded_admin.rs"),
    );
}

#[test]
fn arg_dos_detector_flags_only_uncapped_argument_iteration() {
    let findings = RuleRegistry::with_default_rules()
        .run_by_name(include_str!("fixtures/detectors/arg_dos.rs"), "arg_dos");

    assert_eq!(findings.len(), 1, "{findings:#?}");
    assert_eq!(findings[0].rule_name, "SANCT_ARG_DOS");
    assert!(findings[0].message.contains("recipients"));
    assert!(findings[0].location.contains("uncapped_vec_airdrop"));
}

#[test]
fn sanct_unwrap_detector_is_registered_in_default_rules() {
    let findings = RuleRegistry::with_default_rules().run_by_name(
        include_str!("fixtures/detectors/sanct_unwrap.rs"),
        "sanct_unwrap",
    );

    assert_eq!(findings.len(), 3, "{findings:#?}");
    assert!(findings
        .iter()
        .all(|finding| finding.rule_name == "SANCT_UNWRAP"));
}
