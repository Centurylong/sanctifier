// Comprehensive tests for reentrancy pattern detection

#[cfg(test)]
mod reentrancy_pattern_tests {
    use crate::reentrancy::{ReentrancyIssue, ReentrancyVisitor, RiskyPattern};
    use syn::visit::Visit;

    fn scan(src: &str) -> Vec<ReentrancyIssue> {
        let file: syn::File = syn::parse_str(src).unwrap();
        let mut visitor = ReentrancyVisitor::new();
        visitor.visit_file(&file);
        visitor.issues
    }

    // ── Basic Pattern Tests ──────────────────────────────────────────────────

    #[test]
    fn test_safe_function_with_guard() {
        let src = r#"
            #[contract] pub struct Safe;
            #[contractimpl]
            impl Safe {
                pub fn guarded_transfer(env: Env, nonce: u64, to: Address, amount: i128) {
                    guardian.enter(nonce);
                    env.storage().instance().set(&"balance", &amount);
                    token_client.transfer(&env.current_contract_address(), &to, &amount);
                    guardian.exit();
                }
            }
        "#;
        let issues = scan(src);
        assert!(
            issues.is_empty(),
            "Properly guarded function should have no issues"
        );
    }

    #[test]
    fn test_classic_reentrancy_pattern() {
        let src = r#"
            #[contract] pub struct Classic;
            #[contractimpl]
            impl Classic {
                pub fn withdraw(env: Env, amount: i128) {
                    let balance: i128 = env.storage().instance().get(&"balance").unwrap();
                    env.storage().instance().set(&"balance", &(balance - amount));
                    token_client.transfer(&env.current_contract_address(), &caller, &amount);
                }
            }
        "#;
        let issues = scan(src);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].function_name, "withdraw");
        assert!(issues[0].issue_type.contains("reentrancy"));
        assert_eq!(issues[0].pattern, RiskyPattern::StateBeforeCall);
    }

    // ── Loop Pattern Tests ────────────────────────────────────────────────────

    #[test]
    fn test_external_call_in_for_loop() {
        let src = r#"
            #[contract] pub struct LoopContract;
            #[contractimpl]
            impl LoopContract {
                pub fn batch_process(env: Env, addresses: Vec<Address>) {
                    for addr in addresses.iter() {
                        external_client.process(&addr);
                    }
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty());
        let loop_issue = issues.iter().find(|i| i.pattern == RiskyPattern::CallInLoop);
        assert!(loop_issue.is_some(), "Should detect external call in loop");
        assert_eq!(loop_issue.unwrap().severity, "high");
    }

    #[test]
    fn test_external_call_in_while_loop() {
        let src = r#"
            #[contract] pub struct WhileLoop;
            #[contractimpl]
            impl WhileLoop {
                pub fn process_until_done(env: Env) {
                    let mut done = false;
                    while !done {
                        done = external_client.check_status();
                    }
                }
            }
        "#;
        let issues = scan(src);
        let loop_issue = issues.iter().find(|i| i.pattern == RiskyPattern::CallInLoop);
        assert!(loop_issue.is_some(), "Should detect external call in while loop");
    }

    #[test]
    fn test_external_call_in_loop_construct() {
        let src = r#"
            #[contract] pub struct InfiniteLoop;
            #[contractimpl]
            impl InfiniteLoop {
                pub fn monitor(env: Env) {
                    loop {
                        let status = external_client.get_status();
                        if status == 1 {
                            break;
                        }
                    }
                }
            }
        "#;
        let issues = scan(src);
        let loop_issue = issues.iter().find(|i| i.pattern == RiskyPattern::CallInLoop);
        assert!(loop_issue.is_some(), "Should detect external call in loop construct");
    }

    // ── Multiple External Calls Tests ─────────────────────────────────────────

    #[test]
    fn test_multiple_external_calls_no_guard() {
        let src = r#"
            #[contract] pub struct MultiCall;
            #[contractimpl]
            impl MultiCall {
                pub fn complex_operation(env: Env) {
                    token_client.approve(&spender, &amount);
                    vault_client.deposit(&amount);
                    oracle_client.update_price(&asset);
                }
            }
        "#;
        let issues = scan(src);
        let multi_call_issue = issues.iter().find(|i| i.pattern == RiskyPattern::MultipleExternalCalls);
        assert!(multi_call_issue.is_some(), "Should detect multiple external calls");
        assert_eq!(multi_call_issue.unwrap().severity, "high");
        assert!(multi_call_issue.unwrap().recommendation.contains("3 external calls"));
    }

    #[test]
    fn test_two_external_calls_triggers_warning() {
        let src = r#"
            #[contract] pub struct TwoCalls;
            #[contractimpl]
            impl TwoCalls {
                pub fn dual_action(env: Env) {
                    client_a.action_one();
                    client_b.action_two();
                }
            }
        "#;
        let issues = scan(src);
        let multi_call_issue = issues.iter().find(|i| i.pattern == RiskyPattern::MultipleExternalCalls);
        assert!(multi_call_issue.is_some(), "Should detect two external calls");
    }

    // ── CEI Violation Tests ───────────────────────────────────────────────────

    #[test]
    fn test_state_mutation_after_external_call() {
        let src = r#"
            #[contract] pub struct StateAfter;
            #[contractimpl]
            impl StateAfter {
                pub fn risky_flow(env: Env) {
                    external_client.transfer(&to, &amount);
                    env.storage().instance().set(&"last_transfer", &amount);
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].pattern, RiskyPattern::StateAfterCall);
        assert_eq!(issues[0].severity, "high");
        assert!(issues[0].recommendation.contains("Checks-Effects-Interactions"));
    }

    #[test]
    fn test_critical_cei_violation() {
        let src = r#"
            #[contract] pub struct CriticalCEI;
            #[contractimpl]
            impl CriticalCEI {
                pub fn very_dangerous(env: Env) {
                    env.storage().instance().set(&"status", &1u32);
                    external_client.do_something();
                    env.storage().instance().set(&"completed", &true);
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty());
        let critical = issues.iter().find(|i| i.issue_type == "cei_violation_critical");
        assert!(critical.is_some(), "Should detect critical CEI violation");
        assert_eq!(critical.unwrap().severity, "high");
    }

    #[test]
    fn test_state_before_call_medium_severity() {
        let src = r#"
            #[contract] pub struct StateBefore;
            #[contractimpl]
            impl StateBefore {
                pub fn update_and_call(env: Env) {
                    env.storage().instance().set(&"counter", &42u64);
                    external_client.notify();
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty());
        assert_eq!(issues[0].pattern, RiskyPattern::StateBeforeCall);
        assert_eq!(issues[0].severity, "medium");
    }

    // ── Safe Pattern Tests ─────────────────────────────────────────────────────

    #[test]
    fn test_read_only_operations_safe() {
        let src = r#"
            #[contract] pub struct ReadOnly;
            #[contractimpl]
            impl ReadOnly {
                pub fn query_data(env: Env) -> u64 {
                    let value: u64 = env.storage().instance().get(&"data").unwrap_or(0);
                    let price = external_client.get_price(&asset);
                    value + price
                }
            }
        "#;
        let issues = scan(src);
        assert!(issues.is_empty(), "Read-only operations should be safe");
    }

    #[test]
    fn test_no_external_calls_safe() {
        let src = r#"
            #[contract] pub struct Internal;
            #[contractimpl]
            impl Internal {
                pub fn internal_update(env: Env, value: u64) {
                    env.storage().instance().set(&"value", &value);
                    env.storage().persistent().set(&"timestamp", &env.ledger().timestamp());
                }
            }
        "#;
        let issues = scan(src);
        assert!(issues.is_empty(), "Internal-only operations should be safe");
    }

    #[test]
    fn test_invoke_contract_detected() {
        let src = r#"
            #[contract] pub struct InvokeContract;
            #[contractimpl]
            impl InvokeContract {
                pub fn call_other(env: Env, contract_id: Address) {
                    env.storage().instance().set(&"caller", &contract_id);
                    invoke_contract(&env, &contract_id, &symbol_short!("method"), &args);
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Should detect invoke_contract calls");
    }

    // ── Edge Cases ─────────────────────────────────────────────────────────────

    #[test]
    fn test_nested_loops_with_external_calls() {
        let src = r#"
            #[contract] pub struct NestedLoops;
            #[contractimpl]
            impl NestedLoops {
                pub fn nested_process(env: Env, matrix: Vec<Vec<Address>>) {
                    for row in matrix.iter() {
                        for addr in row.iter() {
                            external_client.process(&addr);
                        }
                    }
                }
            }
        "#;
        let issues = scan(src);
        let loop_issue = issues.iter().find(|i| i.pattern == RiskyPattern::CallInLoop);
        assert!(loop_issue.is_some(), "Should detect external calls in nested loops");
    }

    #[test]
    fn test_multiple_storage_types() {
        let src = r#"
            #[contract] pub struct MultiStorage;
            #[contractimpl]
            impl MultiStorage {
                pub fn update_all(env: Env) {
                    env.storage().instance().set(&"instance_key", &1u32);
                    env.storage().persistent().set(&"persistent_key", &2u32);
                    env.storage().temporary().set(&"temp_key", &3u32);
                    external_client.sync();
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Should detect state mutations across storage types");
    }

    #[test]
    fn test_guard_with_different_naming() {
        let src = r#"
            #[contract] pub struct GuardVariants;
            #[contractimpl]
            impl GuardVariants {
                pub fn with_reentrancy_lock(env: Env, nonce: u64) {
                    reentrancy_lock.enter(nonce);
                    env.storage().instance().set(&"data", &42u64);
                    external_client.call();
                    reentrancy_lock.exit();
                }
            }
        "#;
        let issues = scan(src);
        assert!(issues.is_empty(), "Should recognize reentrancy guard with different naming");
    }

    // ── Complex Scenarios ──────────────────────────────────────────────────────

    #[test]
    fn test_conditional_external_calls() {
        let src = r#"
            #[contract] pub struct Conditional;
            #[contractimpl]
            impl Conditional {
                pub fn conditional_call(env: Env, should_call: bool) {
                    env.storage().instance().set(&"flag", &should_call);
                    if should_call {
                        external_client.execute();
                    }
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Should detect conditional external calls");
    }

    #[test]
    fn test_match_expression_with_calls() {
        let src = r#"
            #[contract] pub struct MatchExpr;
            #[contractimpl]
            impl MatchExpr {
                pub fn match_and_call(env: Env, action: u32) {
                    env.storage().instance().set(&"action", &action);
                    match action {
                        1 => client_a.action_one(),
                        2 => client_b.action_two(),
                        _ => {}
                    }
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Should detect external calls in match expressions");
    }

    #[test]
    fn test_proper_cei_pattern_safe() {
        let src = r#"
            #[contract] pub struct ProperCEI;
            #[contractimpl]
            impl ProperCEI {
                pub fn proper_withdraw(env: Env, amount: i128) {
                    // Checks
                    let balance: i128 = env.storage().instance().get(&"balance").unwrap();
                    assert!(balance >= amount);
                    
                    // Effects
                    env.storage().instance().set(&"balance", &(balance - amount));
                    
                    // Interactions (but without guard, still risky)
                    token_client.transfer(&env.current_contract_address(), &caller, &amount);
                }
            }
        "#;
        let issues = scan(src);
        // This should still trigger a warning because there's no guard,
        // even though it follows CEI pattern
        assert!(!issues.is_empty());
        assert_eq!(issues[0].pattern, RiskyPattern::StateBeforeCall);
    }

    // ── Severity Level Tests ───────────────────────────────────────────────────

    #[test]
    fn test_severity_levels_correct() {
        let high_severity_src = r#"
            #[contract] pub struct HighSeverity;
            #[contractimpl]
            impl HighSeverity {
                pub fn high_risk(env: Env) {
                    for i in 0..10 {
                        external_client.call();
                    }
                }
            }
        "#;
        let high_issues = scan(high_severity_src);
        assert!(high_issues.iter().any(|i| i.severity == "high"));

        let medium_severity_src = r#"
            #[contract] pub struct MediumSeverity;
            #[contractimpl]
            impl MediumSeverity {
                pub fn medium_risk(env: Env) {
                    env.storage().instance().set(&"data", &1u32);
                    external_client.call();
                }
            }
        "#;
        let medium_issues = scan(medium_severity_src);
        assert!(medium_issues.iter().any(|i| i.severity == "medium"));
    }
}
