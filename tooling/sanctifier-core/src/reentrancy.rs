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
}

/// AST visitor that identifies functions which mutate contract state, perform
/// external calls, but do NOT call a `ReentrancyGuardian.enter(...)` or any
/// other nonce-based guard before the mutation/call sequence.
pub struct ReentrancyVisitor {
    pub issues: Vec<ReentrancyIssue>,
    current_fn: Option<String>,
    has_external_call: bool,
    has_state_mutation: bool,
    has_reentrancy_guard: bool,
}

impl ReentrancyVisitor {
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            current_fn: None,
            has_external_call: false,
            has_state_mutation: false,
            has_reentrancy_guard: false,
        }
    }

    fn reset_state(&mut self) {
        self.has_external_call = false;
        self.has_state_mutation = false;
        self.has_reentrancy_guard = false;
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

        // Flag functions that mutate state AND call external contracts without a guard.
        if self.has_external_call && self.has_state_mutation && !self.has_reentrancy_guard {
            self.issues.push(ReentrancyIssue {
                function_name: fn_name.clone(),
                issue_type: "missing_reentrancy_guardian".to_string(),
                location: fn_name,
                recommendation: concat!(
                    "Use `ReentrancyGuardian.enter(nonce)` / `.exit()` to protect ",
                    "state-mutating functions that perform external calls."
                )
                .to_string(),
            });
        }

        self.current_fn = None;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let fn_name = i.sig.ident.to_string();
        self.current_fn = Some(fn_name.clone());
        self.reset_state();

        visit::visit_impl_item_fn(self, i);

        if self.has_external_call && self.has_state_mutation && !self.has_reentrancy_guard {
            self.issues.push(ReentrancyIssue {
                function_name: fn_name.clone(),
                issue_type: "missing_reentrancy_guardian".to_string(),
                location: fn_name,
                recommendation: concat!(
                    "Use `ReentrancyGuardian.enter(nonce)` / `.exit()` to protect ",
                    "state-mutating functions that perform external calls."
                )
                .to_string(),
            });
        }

        self.current_fn = None;
    }

    fn visit_expr_method_call(&mut self, i: &'ast ExprMethodCall) {
        let method = i.method.to_string();

        // Detect state mutations: storage.instance().set / .remove / .update
        if matches!(method.as_str(), "set" | "remove" | "update") {
            // Walk up the receiver chain to check if it's storage-related
            if receiver_contains_storage(&i.receiver) {
                self.has_state_mutation = true;
            }
        }

        // Detect reentrancy guard calls: guardian.enter(...) patterns
        if method == "enter" || method == "exit" {
            if receiver_contains_guard(&i.receiver) {
                self.has_reentrancy_guard = true;
            }
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
        ) {
            if receiver_contains_client(&i.receiver) {
                self.has_external_call = true;
            }
        }

        visit::visit_expr_method_call(self, i);
    }

    fn visit_expr_call(&mut self, i: &'ast ExprCall) {
        // Detect `invoke_contract` / `invoke_contract_check_auth` free-function calls
        if let Expr::Path(p) = &*i.func {
            if let Some(seg) = p.path.segments.last() {
                let name = seg.ident.to_string();
                if name == "invoke_contract" || name == "invoke_contract_check_auth" {
                    self.has_external_call = true;
                }
            }
        }
        visit::visit_expr_call(self, i);
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
                quote::quote!(#(&f.member)).to_string()
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
}
