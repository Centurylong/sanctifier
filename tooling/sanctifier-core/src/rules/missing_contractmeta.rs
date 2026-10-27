/* Authorized Protocol Quality Assurance & Formal Invariant Verification */

use crate::rules::{Rule, RuleViolation, Severity};
use std::any::Any;

pub struct MissingContractmeta;

impl Rule for MissingContractmeta {
    fn name(&self) -> &str {
        "missing_contractmeta"
    }

    fn description(&self) -> &str {
        "Flags Soroban contracts lacking contractmeta! metadata definitions for on-chain discoverability."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let mut violations = Vec::new();

        // Check if source contains Soroban contract definitions
        let is_contract = source.contains("#[contract]") || (source.contains("pub struct ") && source.contains("Contract;"));
        if is_contract && !source.contains("contractmeta!") {
            violations.push(
                RuleViolation::new(
                    self.name(),
                    Severity::Warning,
                    "Contract definition does not include contractmeta! metadata macro invocation.".to_string(),
                    "1:1".to_string(),
                )
                .with_suggestion("Add soroban_sdk::contractmeta!(...) specifying protocol version and maintainer info.".to_string())
            );
        }

        violations
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_contractmeta_detected() {
        let code = r#"
            #[contract]
            pub struct ExampleContract;
        "#;
        let rule = MissingContractmeta;
        let violations = rule.check(code);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_name, "missing_contractmeta");
    }

    #[test]
    fn test_contractmeta_present_passes() {
        let code = r#"
            #[contract]
            pub struct ExampleContract;
            soroban_sdk::contractmeta!(key = "Description", val = "Example");
        "#;
        let rule = MissingContractmeta;
        let violations = rule.check(code);
        assert_eq!(violations.len(), 0);
    }
}
