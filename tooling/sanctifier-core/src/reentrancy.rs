use serde::{Deserialize, Serialize};
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprMethodCall, ItemFn};

/// A potential reentrancy vulnerability identified in source code.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReentrancyIssue {
    /// Contract function in which the risk was detected.
    pub function_name: String,
    /// Category of the issue (e.g. `"missing_reentrancy_guardian"`).
    pub issue_type: String,
    /// Human-readable location: `"<function_name>"`.
    pub location: String,
    /// Actionable recommendation for the developer.
    pub recommendation: String,
    /// Severity level: "high", "medium", "low"
    pub severity: String,
    /// Specific risky pattern detected
    pub pattern: RiskyPattern,
}

/// Types of risky call patterns that can lead to reentrancy
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum RiskyPattern {
    /// State mutation followed by external call (classic CEI violation)
    StateBeforeCall,
    /// Multiple external calls in sequence without guards
    MultipleExternalCalls,
    /// External call in a loop
    CallInLoop,
    /// State mutation after external call (checks-effects-interactions violation)
    StateAfterCall,
    /// Missing reentrancy guard entirely
    NoGuard,
}

/// AST visitor that identifies functions which mutate contract state, perform
/// external calls, but do NOT call a `ReentrancyGuardian.enter(...)` or any
/// other nonce-based guard before the mutation/call sequence.
/// 
/// Enhanced to detect multiple risky patterns:
/// - State mutations before external calls (CEI violation)
/// - Multiple external calls without guards
/// - External calls in loops
/// - State mutations after external calls
pub struct ReentrancyVisitor {
    pub issues: Vec<ReentrancyIssue>,
    current_fn: Option<String>,
    has_external_call: bool,
    has_state_mutation: bool,
    has_reentrancy_guard: bool,
    // Enhanced tracking for pattern detection
    external_call_count: usize,
    state_mutation_positions: Vec<usize>,
    external_call_positions: Vec<usize>,
    statement_counter: usize,
    in_loop: bool,
    external_call_in_loop: bool,
}

impl ReentrancyVisitor {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            current_fn: None,
            has_external_call: false,
            has_state_mutation: false,
            has_reentrancy_guard: false,
            external_call_count: 0,
            state_mutation_positions: Vec::new(),
            external_call_positions: Vec::new(),
            statement_counter: 0,
            in_loop: false,
            external_call_in_loop: false,
        }
    }

    fn reset_state(&mut self) {
        self.has_external_call = false;
        self.has_state_mutation = false;
        self.has_reentrancy_guard = false;
        self.external_call_count = 0;
        self.state_mutation_positions.clear();
        self.external_call_positions.clear();
        self.statement_counter = 0;
        self.in_loop = false;
        self.external_call_in_loop = false;
    }

    /// Analyze patterns and generate appropriate issues
    fn analyze_patterns(&mut self, fn_name: &str) {
        // Pattern 1: External call in loop (highest severity)
        if self.external_call_in_loop {
            self.issues.push(ReentrancyIssue {
                function_name: fn_name.to_string(),
                issue_type: "external_call_in_loop".to_string(),
                location: fn_name.to_string(),
                recommendation: "External calls in loops can lead to reentrancy attacks and gas issues. Consider batching operations or using a reentrancy guard.".to_string(),
                severity: "high".to_string(),
                pattern: RiskyPattern::CallInLoop,
            });
        }

        // Pattern 2: Multiple external calls without guard (high severity)
        if self.external_call_count > 1 && !self.has_reentrancy_guard {
            self.issues.push(ReentrancyIssue {
                function_name: fn_name.to_string(),
                issue_type: "multiple_external_calls".to_string(),
                location: fn_name.to_string(),
                recommendation: format!(
                    "Function makes {} external calls without reentrancy protection. Use ReentrancyGuardian.enter(nonce) / .exit() to protect this function.",
                    self.external_call_count
                ),
                severity: "high".to_string(),
                pattern: RiskyPattern::MultipleExternalCalls,
            });
        }

        // Pattern 3: State mutation before external call (CEI violation)
        if self.has_state_mutation && self.has_external_call && !self.has_reentrancy_guard {
            let state_before_call = self.state_mutation_positions.iter()
                .any(|&state_pos| {
                    self.external_call_positions.iter()
                        .any(|&call_pos| state_pos < call_pos)
                });

            let state_after_call = self.state_mutation_positions.iter()
                .any(|&state_pos| {
                    self.external_call_positions.iter()
                        .any(|&call_pos| state_pos > call_pos)
                });

            if state_before_call && state_after_call {
                // State mutations both before and after - worst case
                self.issues.push(ReentrancyIssue {
                    function_name: fn_name.to_string(),
                    issue_type: "cei_violation_critical".to_string(),
                    location: fn_name.to_string(),
                    recommendation: "State mutations occur both before and after external calls. Follow Checks-Effects-Interactions pattern: perform all state changes before external calls, or use ReentrancyGuardian.".to_string(),
                    severity: "high".to_string(),
                    pattern: RiskyPattern::StateAfterCall,
                });
            } else if state_after_call {
                // State mutation after call
                self.issues.push(ReentrancyIssue {
                    function_name: fn_name.to_string(),
                    issue_type: "state_after_call".to_string(),
                    location: fn_name.to_string(),
                    recommendation: "State mutation after external call violates Checks-Effects-Interactions pattern. Move state changes before the external call or use ReentrancyGuardian.".to_string(),
                    severity: "high".to_string(),
                    pattern: RiskyPattern::StateAfterCall,
                });
            } else if state_before_call {
                // State mutation before call (classic pattern)
                self.issues.push(ReentrancyIssue {
                    function_name: fn_name.to_string(),
                    issue_type: "missing_reentrancy_guardian".to_string(),
                    location: fn_name.to_string(),
                    recommendation: "Use ReentrancyGuardian.enter(nonce) / .exit() to protect state-mutating functions that perform external calls.".to_string(),
                    severity: "medium".to_string(),
                    pattern: RiskyPattern::StateBeforeCall,
                });
            }
        }

        // Pattern 4: No guard at all when both state mutation and external call exist
        if self.has_external_call && self.has_state_mutation && !self.has_reentrancy_guard 
            && self.issues.is_empty() {
            self.issues.push(ReentrancyIssue {
                function_name: fn_name.to_string(),
                issue_type: "missing_reentrancy_guardian".to_string(),
                location: fn_name.to_string(),
                recommendation: "Use ReentrancyGuardian.enter(nonce) / .exit() to protect state-mutating functions that perform external calls.".to_string(),
                severity: "medium".to_string(),
                pattern: RiskyPattern::NoGuard,
            });
        }
    }
}

impl Default for ReentrancyVisitor {
    fn default() -> Self {
        Self::new()
    }
}

impl<'ast> Visit<'ast> for ReentrancyVisitor {
    fn visit_item_fn(&mut self, i: &'ast ItemFn) {
        let fn_name = i.sig.ident.to_string();
        self.current_fn = Some(fn_name.clone());
        self.reset_state();

        visit::visit_item_fn(self, i);

        // Analyze patterns and generate issues
        self.analyze_patterns(&fn_name);

        self.current_fn = None;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let fn_name = i.sig.ident.to_string();
        self.current_fn = Some(fn_name.clone());
        self.reset_state();

        visit::visit_impl_item_fn(self, i);

        // Analyze patterns and generate issues
        self.analyze_patterns(&fn_name);

        self.current_fn = None;
    }

    fn visit_expr_method_call(&mut self, i: &'ast ExprMethodCall) {
        let method = i.method.to_string();

        // Increment statement counter for position tracking
        self.statement_counter += 1;

        // Detect state mutations: storage.instance().set / .remove / .update
        if matches!(method.as_str(), "set" | "remove" | "update") {
            // Walk up the receiver chain to check if it's storage-related
            if receiver_contains_storage(&i.receiver) {
                self.has_state_mutation = true;
                self.state_mutation_positions.push(self.statement_counter);
            }
        }

        // Detect reentrancy guard calls: guardian.enter(...) patterns
        if (method == "enter" || method == "exit") && receiver_contains_guard(&i.receiver) {
            self.has_reentrancy_guard = true;
        }

        // Detect external cross-contract calls via a generated *Client struct
        // Pattern: client.some_fn(...) where receiver contains "client" or "Client"
        if !matches!(
            method.as_str(),
            "set"
                | "get"
                | "has"
                | "remove"
                | "update"
                | "require_auth"
                | "require_auth_for_args"
                | "events"
                | "storage"
                | "instance"
                | "persistent"
                | "temporary"
                | "publish"
                | "ledger"
                | "deployer"
                | "call_as"
                | "try_call"
                | "enter"
                | "exit"
                | "get_nonce"
                | "init"
        ) && receiver_contains_client(&i.receiver)
        {
            self.has_external_call = true;
            self.external_call_count += 1;
            self.external_call_positions.push(self.statement_counter);
            
            if self.in_loop {
                self.external_call_in_loop = true;
            }
        }

        visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        self.statement_counter += 1;

        // Detect `invoke_contract` / `invoke_contract_check_auth` free-function calls
        if let Expr::Path(p) = &*i.func {
            if let Some(seg) = p.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "invoke_contract" || name == "invoke_contract_check_auth" {
                    self.has_external_call = true;
                    self.external_call_count += 1;
                    self.external_call_positions.push(self.statement_counter);
                    
                    if self.in_loop {
                        self.external_call_in_loop = true;
                    }
                }
            }
        }
        visit::visit_expr_call(self, i);
    }

    // Track loop contexts
    fn visit_expr_for_loop(&mut self, i: &'ast syn::ExprForLoop) {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        visit::visit_expr_for_loop(self, i);
        self.in_loop = was_in_loop;
    }

    fn visit_expr_while(&mut self, i: &'ast syn::ExprWhile) {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        visit::visit_expr_while(self, i);
        self.in_loop = was_in_loop;
    }

    fn visit_expr_loop(&mut self, i: &'ast syn::ExprLoop) {
        let was_in_loop = self.in_loop;
        self.in_loop = true;
        visit::visit_expr_loop(self, i);
        self.in_loop = was_in_loop;
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn expr_to_string(expr: &Expr) -> String {
    match expr {
        Expr::MethodCall(m) => {
            format!("{}.{}", expr_to_string(&m.receiver), m.method)
        }
        Expr::Path(p) => p
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::"),
        Expr::Field(f) => {
            format!(
                "{}.{}",
                expr_to_string(&f.base),
                quote::quote!(#(&f.member))
            )
        }
        _ => String::new(),
    }
}

fn receiver_contains_storage(expr: &Expr) -> bool {
    let s = expr_to_string(expr).to_lowercase();
    s.contains("storage")
        || s.contains("instance")
        || s.contains("persistent")
        || s.contains("temporary")
}

fn receiver_contains_guard(expr: &Expr) -> bool {
    let s = expr_to_string(expr).to_lowercase();
    s.contains("guardian") || s.contains("guard") || s.contains("reentrancy")
}

fn receiver_contains_client(expr: &Expr) -> bool {
    let s = expr_to_string(expr).to_lowercase();
    s.contains("client")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use syn::visit::Visit;

    fn scan(src: &str) -> Vec<ReentrancyIssue> {
        let file: syn::File = syn::parse_str(src).unwrap();
        let mut visitor = ReentrancyVisitor::new();
        visitor.visit_file(&file);
        visitor.issues
    }

    #[test]
    fn test_no_issue_for_safe_fn() {
        let src = r#"
            #[contract] pub struct Safe;
            #[contractimpl]
            impl Safe {
                pub fn guarded(env: Env, nonce: u64) {
                    guardian.enter(nonce);
                    env.storage().instance().set(&"key", &42u64);
                    guardian.exit();
                }
            }
        "#;
        let issues = scan(src);
        assert!(
            issues.is_empty(),
            "Expected no issues for guarded function, got: {:?}",
            issues
        );
    }

    #[test]
    fn test_detects_missing_guard() {
        let src = r#"
            #[contract] pub struct Risky;
            #[contractimpl]
            impl Risky {
                pub fn dangerous(env: Env) {
                    env.storage().instance().set(&"balance", &100u64);
                    external_client.transfer(&dest, &amount);
                }
            }
        "#;
        let issues = scan(src);
        assert!(
            !issues.is_empty(),
            "Expected reentrancy issue for unguarded state+external-call function"
        );
        assert_eq!(issues[0].function_name, "dangerous");
        assert_eq!(issues[0].issue_type, "missing_reentrancy_guardian");
    }

    #[test]
    fn test_no_issue_when_no_external_call() {
        let src = r#"
            #[contract] pub struct Standalone;
            #[contractimpl]
            impl Standalone {
                pub fn internal_only(env: Env) {
                    env.storage().instance().set(&"count", &1u64);
                }
            }
        "#;
        let issues = scan(src);
        assert!(
            issues.is_empty(),
            "No external call, so no reentrancy risk expected"
        );
    }

    #[test]
    fn test_detects_external_call_in_loop() {
        let src = r#"
            #[contract] pub struct LoopRisk;
            #[contractimpl]
            impl LoopRisk {
                pub fn batch_transfer(env: Env, recipients: Vec<Address>) {
                    for recipient in recipients.iter() {
                        token_client.transfer(&env.current_contract_address(), &recipient, &100);
                    }
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Expected issue for external call in loop");
        assert_eq!(issues[0].issue_type, "external_call_in_loop");
        assert_eq!(issues[0].severity, "high");
        assert_eq!(issues[0].pattern, RiskyPattern::CallInLoop);
    }

    #[test]
    fn test_detects_multiple_external_calls() {
        let src = r#"
            #[contract] pub struct MultiCall;
            #[contractimpl]
            impl MultiCall {
                pub fn complex_operation(env: Env) {
                    token_client.transfer(&from, &to, &100);
                    oracle_client.get_price(&asset);
                    vault_client.deposit(&amount);
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Expected issue for multiple external calls");
        assert_eq!(issues[0].issue_type, "multiple_external_calls");
        assert_eq!(issues[0].severity, "high");
        assert_eq!(issues[0].pattern, RiskyPattern::MultipleExternalCalls);
    }

    #[test]
    fn test_detects_state_after_call() {
        let src = r#"
            #[contract] pub struct StateAfter;
            #[contractimpl]
            impl StateAfter {
                pub fn risky_update(env: Env) {
                    external_client.do_something();
                    env.storage().instance().set(&"status", &1u32);
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Expected issue for state mutation after external call");
        assert_eq!(issues[0].issue_type, "state_after_call");
        assert_eq!(issues[0].severity, "high");
        assert_eq!(issues[0].pattern, RiskyPattern::StateAfterCall);
    }

    #[test]
    fn test_detects_cei_violation_critical() {
        let src = r#"
            #[contract] pub struct CEIViolation;
            #[contractimpl]
            impl CEIViolation {
                pub fn very_risky(env: Env) {
                    env.storage().instance().set(&"before", &1u32);
                    external_client.transfer(&dest, &amount);
                    env.storage().instance().set(&"after", &2u32);
                }
            }
        "#;
        let issues = scan(src);
        assert!(!issues.is_empty(), "Expected critical CEI violation");
        assert_eq!(issues[0].issue_type, "cei_violation_critical");
        assert_eq!(issues[0].severity, "high");
        assert_eq!(issues[0].pattern, RiskyPattern::StateAfterCall);
    }

    #[test]
    fn test_no_issue_for_read_only() {
        let src = r#"
            #[contract] pub struct ReadOnly;
            #[contractimpl]
            impl ReadOnly {
                pub fn query(env: Env) -> u64 {
                    let value: u64 = env.storage().instance().get(&"key").unwrap_or(0);
                    external_client.get_price(&asset);
                    value
                }
            }
        "#;
        let issues = scan(src);
        assert!(
            issues.is_empty(),
            "No state mutation, so no reentrancy risk"
        );
    }

    // ── Additional Comprehensive Pattern Tests ────────────────────────────────

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
    }

    #[test]
    fn test_critical_cei_violation_both_before_and_after() {
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
    fn test_invoke_contract_detected_as_external_call() {
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
    fn test_conditional_external_calls_detected() {
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
    fn test_guard_with_reentrancy_naming() {
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
}
