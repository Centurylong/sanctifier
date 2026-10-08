use crate::finding_codes::CEI_VIOLATION;
use crate::reentrancy;
use crate::rules::{Rule, RuleViolation, Severity};

/// Detect cross-contract interactions before later storage effects on a
/// reachable syntactic path. Token/SAC clients are external interactions.
/// This is an AST warning, not a whole-program reentrancy proof.
pub struct CeiViolationRule;

impl CeiViolationRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CeiViolationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for CeiViolationRule {
    fn name(&self) -> &str {
        "cei_violation"
    }

    fn description(&self) -> &str {
        "Detects cross-contract or token/SAC interactions before later storage writes"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        reentrancy::models(source)
            .into_iter()
            .flat_map(|model| {
                model.interactions_before_effects().into_iter().map(move |(call, write)| {
                    RuleViolation::new(
                        CEI_VIOLATION,
                        Severity::Error,
                        format!(
                            "{CEI_VIOLATION}: {} invokes external {} on line {} before {} writes state on line {}; a callback may re-enter while internal state is stale",
                            model.function, call.operation, call.line, write.operation, write.line
                        ),
                        format!("{}:{}", model.function, call.line),
                    )
                    .with_suggestion(
                        "Validate and authorize, commit the internal storage update before invoking untrusted contracts/token clients, and apply a reentrancy guard where necessary".to_string(),
                    )
                })
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_sac_transfer_before_storage_write() {
        let source = r#"
            impl Vault {
                pub fn withdraw(e: Env, user: Address, amount: i128, token: Address) {
                    user.require_auth();
                    let t = token::Client::new(&e, &token);
                    t.transfer(&e.current_contract_address(), &user, &amount);
                    e.storage().persistent().set(&user, &amount);
                }
            }
        "#;
        let findings = CeiViolationRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, CEI_VIOLATION);
        assert_eq!(findings[0].severity, Severity::Error);
        assert!(findings[0].message.contains("transfer"));
    }

    #[test]
    fn flags_inline_token_transfer_from() {
        let source = r#"
            impl Vault {
                pub fn deposit(e: Env, from: Address, to: Address, token: Address, amount: i128) {
                    token::Client::new(&e, &token).transfer_from(
                        &e.current_contract_address(), &from, &to, &amount
                    );
                    e.storage().persistent().set(&to, &amount);
                }
            }
        "#;
        let v = CeiViolationRule::new().check(source);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].message.contains("transfer_from"));
    }

    #[test]
    fn flags_host_invoke_before_effect() {
        let source = r#"
            impl Vault {
                pub fn bridge(e: Env, peer: Address) {
                    e.invoke_contract::<()>(&peer, &symbol_short!("run"), Vec::new(&e));
                    e.storage().instance().set(&K::Busy, &true);
                }
            }
        "#;
        let v = CeiViolationRule::new().check(source);
        assert_eq!(v.len(), 1, "{v:#?}");
    }

    #[test]
    fn ignores_effect_before_token_interaction() {
        let source = r#"
            impl Vault {
                pub fn safe_withdraw(e: Env, to: Address, amount: i128, token: Address) {
                    e.storage().persistent().set(&to, &amount);
                    let t = token::Client::new(&e, &token);
                    t.transfer(&e.current_contract_address(), &to, &amount);
                }
            }
        "#;
        assert!(CeiViolationRule::new().check(source).is_empty());
    }

    #[test]
    fn respects_mutually_exclusive_branches() {
        let source = r#"
            impl Vault {
                pub fn conditional(e: Env, peer: Address, should_call: bool) {
                    if should_call {
                        e.invoke_contract::<()>(&peer, &symbol_short!("ping"), Vec::new(&e));
                    } else {
                        e.storage().persistent().set(&K::Value, &1);
                    }
                }
            }
        "#;
        let v = CeiViolationRule::new().check(source);
        assert!(v.is_empty(), "exclusive branches must not combine: {v:#?}");
    }

    #[test]
    fn flags_interaction_and_write_in_same_branch() {
        let source = r#"
            impl Vault {
                pub fn conditional(e: Env, peer: Address, enabled: bool) {
                    if enabled {
                        e.invoke_contract::<()>(&peer, &symbol_short!("ping"), Vec::new(&e));
                        e.storage().persistent().set(&K::Value, &1);
                    } else {
                        e.storage().persistent().set(&K::Other, &2);
                    }
                }
            }
        "#;
        let v = CeiViolationRule::new().check(source);
        assert_eq!(v.len(), 1, "{v:#?}");
    }

    #[test]
    fn ignores_unrelated_method_names_without_storage_or_client_receiver() {
        let source = r#"
            impl Vault {
                pub fn local(e: Env, mut cache: Map<i32, i32>, manager: Handler) {
                    manager.transfer();
                    cache.set(1, 2);
                }
            }
        "#;
        let v = CeiViolationRule::new().check(source);
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn recognizes_bound_storage_handle_and_stops_after_return() {
        let source = r#"
            impl Vault {
                pub fn write_after(e: Env, peer: Address, early: bool) {
                    let store = e.storage().persistent();
                    e.invoke_contract::<()>(&peer, &symbol_short!("ping"), Vec::new(&e));
                    if early { return; }
                    store.set(&K::Value, &1);
                }
            }
        "#;
        let v = CeiViolationRule::new().check(source);
        assert_eq!(v.len(), 1, "{v:#?}");
    }

    #[test]
    fn model_exposes_separate_effects_interactions_and_loops() {
        let source = r#"
            fn main(e: Env, p: Address) {
                for _i in 0..2 {
                    e.invoke_contract::<()>(&p, &symbol_short!("ping"), Vec::new(&e));
                }
                e.storage().instance().set(&K::Value, &1);
            }
        "#;
        let models = reentrancy::models(source);
        assert_eq!(models.len(), 1);
        let model = &models[0];
        assert_eq!(model.effects().len(), 2);
        assert_eq!(model.interactions().len(), 1);
        assert!(model.paths.iter().all(|path| path.loops_present));
    }
}
