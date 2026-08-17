pub mod arg_dos;
pub mod arithmetic_overflow;
pub mod auth_gap;
pub mod edge_amount;
pub mod error_code_collision;
pub mod fee_rounding;
pub mod hardcoded_addr;
pub mod ledger_size;
pub mod missing_ttl;
pub mod panic_detection;
pub mod sanct_unwrap;
pub mod unhandled_result;
pub mod unused_variable;

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
        self.rules
            .iter()
            .flat_map(|rule| rule.check(source))
            .collect()
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
        registry.register(ledger_size::LedgerSizeRule::new());
        registry.register(panic_detection::PanicDetectionRule::new());
        registry.register(arithmetic_overflow::ArithmeticOverflowRule::new());
        registry.register(unhandled_result::UnhandledResultRule::new());
        registry.register(unused_variable::UnusedVariableRule::new());
        // New hygiene rules
        registry.register(hardcoded_addr::HardcodedAddrRule::new());
        registry.register(error_code_collision::ErrorCodeCollisionRule::new());
        registry.register(edge_amount::EdgeAmountRule::new());
        registry.register(fee_rounding::FeeRoundingRule::new());
        registry.register(missing_ttl::MissingTtlRule::new());
        registry.register(arg_dos::ArgDosRule::new());
        registry.register(sanct_unwrap::SanctUnwrapRule::new());
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
