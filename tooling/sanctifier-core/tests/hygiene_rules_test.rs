use sanctifier_core::rules::{
    edge_amount::EdgeAmountRule, error_code_collision::ErrorCodeCollisionRule,
    hardcoded_addr::HardcodedAddrRule, Rule, Severity,
};

const HYGIENE_VIOLATIONS: &str = include_str!("fixtures/hygiene_violations.rs");
const HYGIENE_CLEAN: &str = include_str!("fixtures/hygiene_clean.rs");

#[test]
fn test_hardcoded_addr_detects_violations() {
    let rule = HardcodedAddrRule::new();
    let violations = rule.check(HYGIENE_VIOLATIONS);

    // Should detect hardcoded admin address in initialize
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("address") && v.location.contains("initialize")),
        "Should detect hardcoded admin address"
    );

    // Should detect hardcoded secret key in verify_signature
    let secret_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.message.contains("secret") && v.location.contains("verify_signature"))
        .collect();
    assert!(
        !secret_violations.is_empty(),
        "Should detect hardcoded secret key"
    );

    // Secret key should be Error severity
    assert!(
        secret_violations
            .iter()
            .any(|v| v.severity == Severity::Error),
        "Secret key should be Error severity"
    );
}

#[test]
fn test_hardcoded_addr_allows_clean_code() {
    let rule = HardcodedAddrRule::new();
    let violations = rule.check(HYGIENE_CLEAN);

    // Should not flag the clean initialize function that takes admin as parameter
    assert!(
        !violations.iter().any(|v| v.location.contains("initialize")),
        "Clean code should not trigger hardcoded address violations"
    );
}

#[test]
fn test_error_code_collision_detects_duplicates() {
    let rule = ErrorCodeCollisionRule::new();
    let violations = rule.check(HYGIENE_VIOLATIONS);

    // Should detect duplicate discriminants in ErrorWithDuplicates
    let duplicate_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.message.contains("Duplicate") && v.message.contains("ErrorWithDuplicates"))
        .collect();
    assert!(
        !duplicate_violations.is_empty(),
        "Should detect duplicate discriminants"
    );
    assert_eq!(
        duplicate_violations[0].severity,
        Severity::Error,
        "Duplicate discriminants should be Error severity"
    );
}

#[test]
fn test_error_code_collision_detects_inconsistent_style() {
    let rule = ErrorCodeCollisionRule::new();
    let violations = rule.check(HYGIENE_VIOLATIONS);

    // Should detect inconsistent style in ErrorInconsistent
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("Inconsistent") && v.message.contains("ErrorInconsistent")),
        "Should detect inconsistent discriminant style"
    );
}

#[test]
fn test_error_code_collision_allows_clean_enums() {
    let rule = ErrorCodeCollisionRule::new();
    let violations = rule.check(HYGIENE_CLEAN);

    // Should not flag clean error enums with consistent explicit discriminants
    let error_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.severity == Severity::Error)
        .collect();
    assert!(
        error_violations.is_empty(),
        "Clean error enums should not trigger errors: {:?}",
        error_violations
    );
}

#[test]
fn test_edge_amount_detects_missing_amount_check() {
    let rule = EdgeAmountRule::new();
    let violations = rule.check(HYGIENE_VIOLATIONS);

    // Should detect missing amount check in transfer
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("amount > 0") && v.location.contains("transfer")),
        "Should detect missing amount > 0 check in transfer"
    );

    // Should detect missing amount check in mint
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("amount > 0") && v.location.contains("mint")),
        "Should detect missing amount > 0 check in mint"
    );

    // Should detect missing amount check in burn
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("amount > 0") && v.location.contains("burn")),
        "Should detect missing amount > 0 check in burn"
    );
}

#[test]
fn test_edge_amount_detects_missing_self_transfer_check() {
    let rule = EdgeAmountRule::new();
    let violations = rule.check(HYGIENE_VIOLATIONS);

    // Should detect missing from != to check in transfer
    assert!(
        violations
            .iter()
            .any(|v| v.message.contains("from != to") && v.location.contains("transfer")),
        "Should detect missing self-transfer check"
    );
}

#[test]
fn test_edge_amount_allows_proper_validation() {
    let rule = EdgeAmountRule::new();
    let violations = rule.check(HYGIENE_CLEAN);

    // Should not flag functions with proper validation
    assert!(
        violations.is_empty(),
        "Clean code with proper validation should not trigger violations: {:?}",
        violations
    );
}

#[test]
fn test_all_hygiene_rules_together() {
    let hardcoded_rule = HardcodedAddrRule::new();
    let error_code_rule = ErrorCodeCollisionRule::new();
    let edge_amount_rule = EdgeAmountRule::new();

    let mut all_violations = Vec::new();
    all_violations.extend(hardcoded_rule.check(HYGIENE_VIOLATIONS));
    all_violations.extend(error_code_rule.check(HYGIENE_VIOLATIONS));
    all_violations.extend(edge_amount_rule.check(HYGIENE_VIOLATIONS));

    // Should detect multiple violations
    assert!(
        all_violations.len() >= 7,
        "Should detect at least 7 violations across all rules, found: {}",
        all_violations.len()
    );

    // Print summary
    println!("\n=== Hygiene Violations Summary ===");
    for v in &all_violations {
        println!("[{:?}] {}: {}", v.severity, v.rule_name, v.message);
    }
}

#[test]
fn test_hygiene_rules_respect_severity_levels() {
    let hardcoded_rule = HardcodedAddrRule::new();
    let violations = hardcoded_rule.check(HYGIENE_VIOLATIONS);

    // Secret keys should be Error severity
    let secret_violations: Vec<_> = violations
        .iter()
        .filter(|v| v.message.contains("secret") || v.message.contains("Secret"))
        .collect();
    assert!(
        secret_violations
            .iter()
            .any(|v| v.severity == Severity::Error),
        "Secret key violations should be Error severity"
    );
}
