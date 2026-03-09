#[cfg(test)]
mod tests {
    use crate::rules::RuleEngine;
    use sanctifier_core::{Analyzer, SanctifyConfig};
    
    // ── Macro to helpers ────────────────────────────────────────────────────────
    fn setup_engine() -> (Analyzer, SanctifyConfig) {
        let config = SanctifyConfig::default();
        let analyzer = Analyzer::new(config.clone());
        (analyzer, config)
    }

    #[test]
    fn test_trigger_ledger_size_warning() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #![no_std]
            use soroban_sdk::{contracttype, Bytes};
            #[contracttype]
            pub struct TooBig {
                pub data: [u8; 70000],
            }
        "#;
        let results = engine.run_all(source, None);
        // While our estimator might be simple, 70KB will trigger a warning.
        assert!(!results.size_warnings.is_empty());
    }

    #[test]
    fn test_trigger_unsafe_patterns() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn unsafe_code(env: Env) {
                    let x: Option<u32> = None;
                    let _ = x.unwrap();
                    let _ = x.expect("This will fail");
                }
            }
        "#;
        let results = engine.run_all(source, None);
        assert!(results.unsafe_patterns.len() >= 2);
    }

    #[test]
    fn test_trigger_auth_gaps() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn modify_state(env: Env, val: u32) {
                    // This modifies state but has no require_auth()
                    env.storage().instance().set(&Symbol::new(&env, "key"), &val);
                }
            }
        "#;
        let results = engine.run_all(source, None);
        assert!(!results.auth_gaps.is_empty());
        assert_eq!(results.auth_gaps[0], "modify_state");
    }

    #[test]
    fn test_trigger_panic_issues() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn explicit_panic(env: Env) {
                    panic!("Critical failure");
                }
            }
        "#;
        let results = engine.run_all(source, None);
        assert!(!results.panic_issues.is_empty());
        assert_eq!(results.panic_issues[0].issue_type, "panic!");
    }

    #[test]
    fn test_trigger_arithmetic_issues() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn risky_math(env: Env, a: u64, b: u64) -> u64 {
                    a + b // Bare plus operator
                }
            }
        "#;
        let results = engine.run_all(source, None);
        assert!(!results.arithmetic_issues.is_empty());
        assert_eq!(results.arithmetic_issues[0].operation, "+");
    }

    #[test]
    fn test_trigger_deprecated_apis() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn legacy(env: Env) {
                    env.put_contract_data(&Symbol::new(&env, "key"), &123);
                }
            }
        "#;
        let results = engine.run_all(source, None);
        assert!(!results.deprecated_api_issues.is_empty());
        assert_eq!(results.deprecated_api_issues[0].deprecated_api, "put_contract_data");
    }

    #[test]
    fn test_trigger_reentrancy_risks() {
        let (analyzer, config) = setup_engine();
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn risky_reentrancy(env: Env, client: TokenClient) {
                    // 1. State mutation
                    env.storage().instance().set(&"status", &1u32);
                    // 2. External call
                    client.transfer(&env.current_contract_address(), &to, &amount);
                    // No guard used - triggers reentrancy risk
                }
            }
        "#;
        let results = engine.run_all(source, None);
        assert!(!results.reentrancy_issues.is_empty());
        assert_eq!(results.reentrancy_issues[0].issue_type, "missing_reentrancy_guardian");
    }

    #[test]
    fn test_trigger_custom_rules() {
        let mut config = SanctifyConfig::default();
        config.custom_rules.push(sanctifier_core::CustomRule {
            name: "TODO_FINDER".to_string(),
            pattern: "TODO".to_string(),
        });

        let analyzer = Analyzer::new(config.clone());
        let engine = RuleEngine::new(&analyzer, &config);
        
        let source = r#"
            // TODO: Implement something here
            pub fn placeholder() {}
        "#;
        let results = engine.run_all(source, None);
        assert!(!results.custom_rule_matches.is_empty());
        assert_eq!(results.custom_rule_matches[0].rule_name, "TODO_FINDER");
    }
}
