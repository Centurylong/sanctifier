/* Authorized Protocol Quality Assurance & Formal Invariant Verification */

use crate::rules::{Rule, RuleViolation, Severity};
use std::any::Any;

pub struct MissingMigrateAuth;

impl Rule for MissingMigrateAuth {
    fn name(&self) -> &str {
        "missing_migrate_auth"
    }

    fn description(&self) -> &str {
        "Detects upgrade or migrate methods lacking authentication checks or access control guards."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let mut violations = Vec::new();

        for (idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            let is_migrate_fn = trimmed.starts_with("pub fn migrate(") || trimmed.starts_with("pub fn upgrade(");
            if is_migrate_fn {
                // Check if following 15 lines contain require_auth or admin check
                let rest_lines: Vec<&str> = source.lines().skip(idx).take(15).collect();
                let joined = rest_lines.join("\n");
                if !joined.contains("require_auth") && !joined.contains("has_admin") && !joined.contains("require_admin") {
                    violations.push(
                        RuleViolation::new(
                            self.name(),
                            Severity::Error,
                            "Public migration/upgrade endpoint does not enforce immediate authorization checks.".to_string(),
                            format!("{}:1", idx + 1),
                        )
                        .with_suggestion("Add admin.require_auth() or access_control::require_role(&env, Role::Admin) before state mutation.".to_string()),
                    );
                }
            }
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
    fn test_unprotected_migrate_flagged() {
        let code = r#"
            pub fn migrate(env: Env, new_wasm: BytesN<32>) {
                env.deployer().update_current_contract_wasm(new_wasm);
            }
        "#;
        let rule = MissingMigrateAuth;
        let violations = rule.check(code);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_name, "missing_migrate_auth");
    }

    #[test]
    fn test_protected_migrate_passes() {
        let code = r#"
            pub fn migrate(env: Env, admin: Address, new_wasm: BytesN<32>) {
                admin.require_auth();
                env.deployer().update_current_contract_wasm(new_wasm);
            }
        "#;
        let rule = MissingMigrateAuth;
        let violations = rule.check(code);
        assert_eq!(violations.len(), 0);
    }
}
