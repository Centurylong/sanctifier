use crate::reentrancy::ReentrancyIssue;
use crate::{
    ArithmeticIssue, CustomRuleMatch, DeprecatedApiIssue, PanicIssue, SizeWarning,
    SizeWarningLevel, UnsafePattern, UpgradeReport,
};
use serde::{Deserialize, Serialize};

/// Represents the breakdown of the Sanctity Score calculation.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SanctityScore {
    pub total_score: u32,
    pub security_score: u32,
    pub verification_score: u32,
    pub coverage_score: u32,
    pub deductions: Vec<ScoreDeduction>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScoreDeduction {
    pub category: String,
    pub amount: u32,
    pub message: String,
}

pub struct ScoringInput<'a> {
    pub size_warnings: &'a [SizeWarning],
    pub unsafe_patterns: &'a [UnsafePattern],
    pub auth_gaps: &'a [String],
    pub panic_issues: &'a [PanicIssue],
    pub arithmetic_issues: &'a [ArithmeticIssue],
    pub deprecated_api_issues: &'a [DeprecatedApiIssue],
    pub custom_rule_matches: &'a [CustomRuleMatch],
    pub reentrancy_issues: &'a [ReentrancyIssue],
    pub upgrade_report: &'a UpgradeReport,
    pub proven_assertions: u32,
    pub total_assertions: u32,
    pub test_coverage: f32, // 0.0 to 1.0
}

/// Computes the Sanctity Score based on analysis hits, formal verification, and test coverage.
pub fn calculate_sanctity_score(input: ScoringInput) -> SanctityScore {
    let mut deductions = Vec::new();
    let mut total_deduction = 0;

    // ── Static Analysis Deductions ──────────────────────────────────────────

    // Critical: Reentrancy Risks
    if !input.reentrancy_issues.is_empty() {
        let amount = (input.reentrancy_issues.len() as u32 * 20).min(40);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "Critical".to_string(),
            amount,
            message: format!(
                "{} Reentrancy risks detected",
                input.reentrancy_issues.len()
            ),
        });
    }

    // Critical/High: Authentication Gaps
    if !input.auth_gaps.is_empty() {
        let amount = (input.auth_gaps.len() as u32 * 15).min(30);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "High".to_string(),
            amount,
            message: format!("{} Authentication gaps identified", input.auth_gaps.len()),
        });
    }

    // High: Arithmetic Overflows
    if !input.arithmetic_issues.is_empty() {
        let amount = (input.arithmetic_issues.len() as u32 * 10).min(20);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "High".to_string(),
            amount,
            message: format!(
                "{} Unchecked arithmetic operations",
                input.arithmetic_issues.len()
            ),
        });
    }

    // High: Explicit Panics/Unwraps
    let unsafe_count = input.unsafe_patterns.len() + input.panic_issues.len();
    if unsafe_count > 0 {
        let amount = (unsafe_count as u32 * 5).min(15);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "High".to_string(),
            amount,
            message: format!("{} Explicit panics or unwraps found", unsafe_count),
        });
    }

    // Medium: Upgrade Issues
    if !input.upgrade_report.findings.is_empty() {
        let amount = (input.upgrade_report.findings.len() as u32 * 5).min(15);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "Medium".to_string(),
            amount,
            message: format!(
                "{} Upgrade safety issues detected",
                input.upgrade_report.findings.len()
            ),
        });
    }

    // Medium/Low: Ledger Size Warnings
    let exceeds = input
        .size_warnings
        .iter()
        .filter(|w| w.level == SizeWarningLevel::ExceedsLimit)
        .count();
    let approaching = input
        .size_warnings
        .iter()
        .filter(|w| w.level == SizeWarningLevel::ApproachingLimit)
        .count();

    if exceeds > 0 {
        let amount = (exceeds as u32 * 10).min(20);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "Medium".to_string(),
            amount,
            message: format!("{} Types exceed ledger size limits", exceeds),
        });
    }
    if approaching > 0 {
        let amount = (approaching as u32 * 3).min(10);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "Low".to_string(),
            amount,
            message: format!("{} Types approaching ledger size limits", approaching),
        });
    }

    // Low: Deprecated APIs
    if !input.deprecated_api_issues.is_empty() {
        let amount = (input.deprecated_api_issues.len() as u32 * 2).min(10);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "Low".to_string(),
            amount,
            message: format!(
                "{} Usage of deprecated Soroban APIs",
                input.deprecated_api_issues.len()
            ),
        });
    }

    // Low: Custom Rule Matches
    if !input.custom_rule_matches.is_empty() {
        let amount = (input.custom_rule_matches.len() as u32 * 2).min(10);
        total_deduction += amount;
        deductions.push(ScoreDeduction {
            category: "Low".to_string(),
            amount,
            message: format!(
                "{} Custom rule violations found",
                input.custom_rule_matches.len()
            ),
        });
    }

    // ── Final Calculation ───────────────────────────────────────────────────

    let security_score = 100u32.saturating_sub(total_deduction);

    let verification_score = if input.total_assertions > 0 {
        ((input.proven_assertions as f32 / input.total_assertions as f32) * 100.0) as u32
    } else {
        0
    };

    let coverage_score = (input.test_coverage * 100.0) as u32;

    // Weighting: Static Analysis (50%), Formal Verification (30%), Test Coverage (20%)
    let total_score = (security_score as f32 * 0.5)
        + (verification_score as f32 * 0.3)
        + (coverage_score as f32 * 0.2);

    SanctityScore {
        total_score: total_score.round() as u32,
        security_score,
        verification_score,
        coverage_score,
        deductions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UpgradeReport;

    #[test]
    fn test_perfect_score() {
        let input = ScoringInput {
            size_warnings: &[],
            unsafe_patterns: &[],
            auth_gaps: &[],
            panic_issues: &[],
            arithmetic_issues: &[],
            deprecated_api_issues: &[],
            custom_rule_matches: &[],
            reentrancy_issues: &[],
            upgrade_report: &UpgradeReport::empty(),
            proven_assertions: 10,
            total_assertions: 10,
            test_coverage: 1.0,
        };

        let score = calculate_sanctity_score(input);
        assert_eq!(score.total_score, 100);
        assert_eq!(score.security_score, 100);
        assert_eq!(score.verification_score, 100);
        assert_eq!(score.coverage_score, 100);
    }

    #[test]
    fn test_score_with_vulnerabilities() {
        // 1 Reentrancy (-20), 1 Auth Gap (-15) = 65 security score
        let input = ScoringInput {
            size_warnings: &[],
            unsafe_patterns: &[],
            auth_gaps: &["gap".to_string()],
            panic_issues: &[],
            arithmetic_issues: &[],
            deprecated_api_issues: &[],
            custom_rule_matches: &[],
            reentrancy_issues: &[crate::reentrancy::ReentrancyIssue {
                function_name: "test".to_string(),
                issue_type: "reentrancy".to_string(),
                location: "loc".to_string(),
                recommendation: "Use a guard".to_string(),
            }],
            upgrade_report: &UpgradeReport::empty(),
            proven_assertions: 10,
            total_assertions: 10,
            test_coverage: 1.0,
        };

        let score = calculate_sanctity_score(input);
        assert_eq!(score.security_score, 65); // 100 - 20 - 15 = 65
                                              // Total = 65 * 0.5 + 100 * 0.3 + 100 * 0.2 = 32.5 + 30 + 20 = 82.5 -> 83
        assert_eq!(score.total_score, 83);
        assert_eq!(score.deductions.len(), 2);
    }

    #[test]
    fn test_score_with_low_verification() {
        let input = ScoringInput {
            size_warnings: &[],
            unsafe_patterns: &[],
            auth_gaps: &[],
            panic_issues: &[],
            arithmetic_issues: &[],
            deprecated_api_issues: &[],
            custom_rule_matches: &[],
            reentrancy_issues: &[],
            upgrade_report: &UpgradeReport::empty(),
            proven_assertions: 0,
            total_assertions: 10,
            test_coverage: 0.5,
        };

        let score = calculate_sanctity_score(input);
        assert_eq!(score.security_score, 100);
        assert_eq!(score.verification_score, 0);
        assert_eq!(score.coverage_score, 50);
        // Total = 100 * 0.5 + 0 * 0.3 + 50 * 0.2 = 50 + 0 + 10 = 60
        assert_eq!(score.total_score, 60);
    }
}
