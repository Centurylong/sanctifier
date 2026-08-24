pub mod allowance_race;
pub mod arg_dos;
pub mod arithmetic_overflow;
pub mod auth_gap;
pub mod balance_equality;
pub mod bls_subgroup_check;
pub mod contracterror_enum;
pub mod cross_contract_call_in_loop;
pub mod division_by_zero;
pub mod eager_unwrap_or;
pub mod edge_amount;
pub mod error_code_collision;
pub mod excessive_clone;
pub mod fee_rounding;
pub mod hardcoded_addr;
pub mod init_hardcoded_admin;
pub mod ledger_seconds;
pub mod ledger_size;
pub mod missing_ttl;
pub mod nullifier_growth;
pub mod panic_detection;
pub mod proof_length_check;
pub mod public_input_range;
pub mod reserve_withdrawal;
pub mod sanct_unwrap;
pub mod sep41_allowance_decrement;
pub mod sep41_approval_expiration;
pub mod shift_overflow;
pub mod state_write_in_view;
pub mod tier_boundary_off_by_one;
pub mod unbounded_event_emission;
pub mod unbounded_return;
pub mod unbounded_storage;
pub mod unhandled_result;
pub mod unsigned_underflow;
pub mod unused_variable;
pub mod vesting_schedule;
pub mod view_panic;
pub mod vk_provenance;
pub mod wrong_auth_args;

use serde::Serialize;
use std::any::Any;

pub trait Rule: Send + Sync + std::panic::UnwindSafe + std::panic::RefUnwindSafe {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn check(&self, source: &str) -> Vec<RuleViolation>;
    fn fix(&self, _source: &str) -> Vec<Patch> {
        vec![]
    }
    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Patch {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub replacement: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleViolation {
    pub rule_name: String,
    pub severity: Severity,
    pub message: String,
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub patches: Vec<Patch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl RuleViolation {
    pub fn new(rule_name: &str, severity: Severity, message: String, location: String) -> Self {
        Self {
            rule_name: rule_name.to_string(),
            severity,
            message,
            location,
            suggestion: None,
            patches: vec![],
        }
    }

    pub fn with_patches(mut self, patches: Vec<Patch>) -> Self {
        self.patches = patches;
        self
    }

    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleTiming {
    pub rule_name: String,
    pub duration: std::time::Duration,
}

pub struct RuleRegistry {
    pub(crate) rules: Vec<Box<dyn Rule>>,
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register<R: Rule + 'static>(&mut self, rule: R) {
        self.rules.push(Box::new(rule));
    }

    pub fn run_all(&self, source: &str) -> Vec<RuleViolation> {
        let mut violations: Vec<RuleViolation> = self
            .rules
            .iter()
            .flat_map(|rule| rule.check(source))
            .collect();

        // Macro-expansion-aware pass: analyse logic hidden behind simple local
        // `macro_rules!` wrappers so it isn't a false negative. The expansion is
        // additive — findings already visible in the original source are
        // de-duplicated by (rule, message), and code with no expandable macros
        // is left completely unchanged.
        if let Some(expanded) = crate::macro_expand::expand_local_macros(source) {
            let mut seen: std::collections::HashSet<(String, String)> = violations
                .iter()
                .map(|v| (v.rule_name.clone(), v.message.clone()))
                .collect();
            for rule in &self.rules {
                for v in rule.check(&expanded) {
                    if seen.insert((v.rule_name.clone(), v.message.clone())) {
                        violations.push(v);
                    }
                }
            }
        }

        // Ensure a deterministic, run-independent ordering: sort by source
        // location first, then rule name, then message. This makes output
        // reproducible regardless of rule registration/iteration order or
        // any future parallel scheduling (e.g. par_iter or concurrent
        // multi-file scans), which matters for diffing scan output,
        // snapshot tests, and CI reproducibility.
        violations.sort_by(|a, b| {
            (&a.location, a.rule_name.as_str(), a.message.as_str()).cmp(&(
                &b.location,
                b.rule_name.as_str(),
                b.message.as_str(),
            ))
        });

        violations
    }

    pub fn run_by_name(&self, source: &str, name: &str) -> Vec<RuleViolation> {
        self.rules
            .iter()
            .filter(|rule| rule.name() == name)
            .flat_map(|rule| rule.check(source))
            .collect()
    }

    /// Like `run_all`, but also returns how long each rule took to run —
    /// useful for spotting a pathologically slow detector on a large file.
    ///
    /// Scope limitation: unlike `run_all`, this does not perform any
    /// macro-expansion second pass — it only measures per-rule cost on the
    /// primary source, which is the useful diagnostic signal for timing.
    pub fn run_all_with_timings(&self, source: &str) -> (Vec<RuleViolation>, Vec<RuleTiming>) {
        let mut violations = Vec::new();
        let mut timings = Vec::new();
        for rule in &self.rules {
            let start = std::time::Instant::now();
            let mut v = rule.check(source);
            timings.push(RuleTiming {
                rule_name: rule.name().to_string(),
                duration: start.elapsed(),
            });
            violations.append(&mut v);
        }
        (violations, timings)
    }

    /// Rules from `timings` whose duration exceeded `threshold` — a simple
    /// slow-rule diagnostic so a pathological detector doesn't silently eat
    /// scan time.
    pub fn slow_rules(timings: &[RuleTiming], threshold: std::time::Duration) -> Vec<&RuleTiming> {
        timings.iter().filter(|t| t.duration > threshold).collect()
    }

    pub fn available_rules(&self) -> Vec<&str> {
        self.rules.iter().map(|rule| rule.name()).collect()
    }

    pub fn with_default_rules() -> Self {
        let mut registry = Self::new();
        registry.register(auth_gap::AuthGapRule::new());
        registry.register(auth_gap::VisibilityLeakRule::new());
        registry.register(ledger_size::LedgerSizeRule::new());
        registry.register(panic_detection::PanicDetectionRule::new());
        registry.register(arithmetic_overflow::ArithmeticOverflowRule::new());
        registry.register(unhandled_result::UnhandledResultRule::new());
        registry.register(unused_variable::UnusedVariableRule::new());
        // New hygiene rules
        registry.register(hardcoded_addr::HardcodedAddrRule::new());
        registry.register(error_code_collision::ErrorCodeCollisionRule::new());
        registry.register(edge_amount::EdgeAmountRule::new());
        registry.register(bls_subgroup_check::BlsSubgroupCheckRule::new());
        registry.register(wrong_auth_args::WrongAuthArgsRule::new());
        registry.register(balance_equality::BalanceEqualityRule::new());
        registry.register(fee_rounding::FeeRoundingRule::new());
        registry.register(excessive_clone::ExcessiveCloneRule::new());
        registry.register(missing_ttl::MissingTtlRule::new());
        registry.register(arg_dos::ArgDosRule::new());
        registry.register(sanct_unwrap::SanctUnwrapRule::new());
        registry.register(init_hardcoded_admin::InitHardcodedAdminRule::new());
        registry.register(shift_overflow::ShiftOverflowRule::new());
        registry.register(unbounded_storage::UnboundedStorageRule::new());
        registry.register(view_panic::ViewPanicRule::new());
        registry.register(allowance_race::AllowanceRaceRule::new());
        registry.register(state_write_in_view::StateWriteInViewRule::new());
        registry.register(division_by_zero::DivisionByZeroRule::new());
        registry.register(eager_unwrap_or::EagerUnwrapOrRule::new());
        registry.register(unsigned_underflow::UnsignedUnderflowRule::new());
        registry.register(ledger_seconds::LedgerSecondsRule::new());
        registry.register(tier_boundary_off_by_one::TierBoundaryOffByOneRule::new());
        registry.register(unbounded_return::UnboundedReturnRule::new());
        registry.register(reserve_withdrawal::ReserveWithdrawalRule::new());
        registry.register(contracterror_enum::ContracterrorEnumRule::new());
        registry.register(vesting_schedule::VestingScheduleRule::new());
        registry.register(cross_contract_call_in_loop::CrossContractCallInLoopRule::new());
        registry.register(unbounded_event_emission::UnboundedEventEmissionRule::new());
        registry.register(sep41_allowance_decrement::Sep41AllowanceDecrementRule::new());
        registry.register(sep41_approval_expiration::Sep41ApprovalExpirationRule::new());
        registry.register(nullifier_growth::NullifierGrowthRule::new());
        registry.register(proof_length_check::ProofLengthCheckRule::new());
        registry.register(vk_provenance::VkProvenanceRule::new());
        registry.register(public_input_range::PublicInputRangeRule::new());
        registry
    }
}

#[cfg(test)]
mod rule_timing_tests {
    use super::*;

    #[test]
    fn run_all_with_timings_returns_one_timing_per_rule_in_order() {
        let registry = RuleRegistry::with_default_rules();
        let (_violations, timings) = registry.run_all_with_timings("fn main() {}");

        assert_eq!(timings.len(), registry.available_rules().len());
        let expected_names: Vec<&str> = registry.available_rules();
        let actual_names: Vec<&str> = timings.iter().map(|t| t.rule_name.as_str()).collect();
        assert_eq!(actual_names, expected_names);
    }

    #[test]
    fn slow_rules_filters_by_threshold() {
        let timings = vec![
            RuleTiming {
                rule_name: "fast_rule".to_string(),
                duration: std::time::Duration::from_millis(1),
            },
            RuleTiming {
                rule_name: "slow_rule".to_string(),
                duration: std::time::Duration::from_millis(100),
            },
        ];

        let threshold = std::time::Duration::from_millis(50);
        let slow = RuleRegistry::slow_rules(&timings, threshold);

        assert_eq!(slow.len(), 1);
        assert_eq!(slow[0].rule_name, "slow_rule");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedRule {
        name: &'static str,
        violations: Vec<RuleViolation>,
    }

    impl Rule for FixedRule {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "test-only rule that returns a fixed set of violations"
        }

        fn check(&self, _source: &str) -> Vec<RuleViolation> {
            self.violations.clone()
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    fn violation(rule_name: &str, message: &str, location: &str) -> RuleViolation {
        RuleViolation::new(
            rule_name,
            Severity::Warning,
            message.to_string(),
            location.to_string(),
        )
    }

    fn as_tuples(violations: &[RuleViolation]) -> Vec<(String, String, String)> {
        violations
            .iter()
            .map(|v| (v.location.clone(), v.rule_name.clone(), v.message.clone()))
            .collect()
    }

    #[test]
    fn run_all_sorts_violations_by_location_then_rule_then_message() {
        let mut registry = RuleRegistry::new();

        // Register rules in an order, and with per-rule violation orders,
        // that are deliberately "wrong" relative to the expected sort key.
        registry.register(FixedRule {
            name: "rule_z",
            violations: vec![violation("rule_z", "z message", "src/lib.rs:9:1")],
        });
        registry.register(FixedRule {
            name: "rule_a",
            violations: vec![
                violation("rule_a", "second", "src/lib.rs:2:5"),
                violation("rule_a", "first", "src/lib.rs:1:1"),
            ],
        });
        registry.register(FixedRule {
            name: "rule_b",
            violations: vec![violation("rule_b", "b message", "src/lib.rs:1:1")],
        });

        let violations = registry.run_all("unused");

        let expected = vec![
            (
                "src/lib.rs:1:1".to_string(),
                "rule_a".to_string(),
                "first".to_string(),
            ),
            (
                "src/lib.rs:1:1".to_string(),
                "rule_b".to_string(),
                "b message".to_string(),
            ),
            (
                "src/lib.rs:2:5".to_string(),
                "rule_a".to_string(),
                "second".to_string(),
            ),
            (
                "src/lib.rs:9:1".to_string(),
                "rule_z".to_string(),
                "z message".to_string(),
            ),
        ];

        assert_eq!(as_tuples(&violations), expected);
    }

    #[test]
    fn run_all_is_deterministic_across_repeated_runs() {
        let mut registry = RuleRegistry::new();
        registry.register(FixedRule {
            name: "rule_z",
            violations: vec![violation("rule_z", "z message", "src/lib.rs:10:1")],
        });
        registry.register(FixedRule {
            name: "rule_a",
            violations: vec![
                violation("rule_a", "second", "src/lib.rs:1:5"),
                violation("rule_a", "first", "src/lib.rs:1:1"),
            ],
        });
        registry.register(FixedRule {
            name: "rule_b",
            violations: vec![violation("rule_b", "b message", "src/lib.rs:1:1")],
        });

        let first_run = registry.run_all("unused");
        let second_run = registry.run_all("unused");

        assert_eq!(as_tuples(&first_run), as_tuples(&second_run));
    }
}
