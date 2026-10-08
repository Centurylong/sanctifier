use crate::finding_codes::STALE_STATE_POSTCALL;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Block, Expr, ExprMethodCall, Pat};

/// Intraprocedural detector for a cached Soroban storage value used in a
/// decision after an external contract call that could mutate the same state.
///
/// The detector tracks local storage reads and definite reassignments, handles
/// conditional branches conservatively, and never treats a fresh post-call
/// storage read as stale. This is an AST heuristic, not interprocedural alias
/// or on-chain reentrancy proof.
pub struct StaleStatePostcallRule;

impl StaleStatePostcallRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StaleStatePostcallRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for StaleStatePostcallRule {
    fn name(&self) -> &str {
        "stale_state_postcall"
    }

    fn description(&self) -> &str {
        "Detects cached storage state used for a decision after an external call without refresh"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Some(file) = crate::parse_cache::parse_cached(source) else {
            return vec![];
        };
        let mut scanner = FileScanner { violations: vec![] };
        scanner.visit_file(&file);
        scanner.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FileScanner {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FileScanner {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        // Test source isn't a contract entrypoint. Prevent diagnostic noise.
        if node.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && quote::quote!(#attr).to_string().contains("test")
        }) {
            return;
        }
        visit::visit_item_mod(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations
            .extend(analyse(&node.sig.ident.to_string(), &node.block));
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations
            .extend(analyse(&node.sig.ident.to_string(), &node.block));
    }
}

fn analyse(name: &str, body: &Block) -> Vec<RuleViolation> {
    let mut flow = FunctionFlow {
        function: name.to_owned(),
        state: FlowState::default(),
        violations: Vec::new(),
        emitted: HashSet::new(),
    };
    flow.visit_block(body);
    flow.violations
}

#[derive(Clone, Default)]
struct FlowState {
    // Local names whose last assignment read storage before the external call.
    cached: HashSet<String>,
    // Local variables initialized from generated Soroban *Client::new.
    clients: HashSet<String>,
    external_seen: bool,
}

impl FlowState {
    fn join(a: &Self, b: &Self) -> Self {
        Self {
            // A stale path is possible if either branch retains the old value.
            cached: a.cached.union(&b.cached).cloned().collect(),
            clients: a.clients.union(&b.clients).cloned().collect(),
            external_seen: a.external_seen || b.external_seen,
        }
    }
}

struct FunctionFlow {
    function: String,
    state: FlowState,
    violations: Vec<RuleViolation>,
    emitted: HashSet<(String, usize)>,
}

impl FunctionFlow {
    fn decision(&mut self, condition: &Expr, line: usize) {
        if !self.state.external_seen {
            return;
        }
        let mut stale: Vec<_> = used_names(condition)
            .into_iter()
            .filter(|name| self.state.cached.contains(name))
            .collect();
        stale.sort();
        for name in stale {
            if !self.emitted.insert((name.clone(), line)) {
                continue;
            }
            self.violations.push(
                RuleViolation::new(
                    STALE_STATE_POSTCALL,
                    Severity::Warning,
                    format!(
                        "{STALE_STATE_POSTCALL}: {} uses cached storage variable {} in a decision after an external call",
                        self.function, name
                    ),
                    format!("{}:{}", self.function, line),
                )
                .with_suggestion(
                    "Read the storage key again after the external call, then base the decision on the refreshed value".into(),
                ),
            );
        }
    }

    fn assign(&mut self, name: &str, rhs: &Expr) {
        let came_from_cache = used_names(rhs)
            .iter()
            .any(|used| self.state.cached.contains(used));
        if has_storage_read(rhs) {
            if self.state.external_seen {
                self.state.cached.remove(name);
            } else {
                self.state.cached.insert(name.to_string());
            }
        } else if came_from_cache {
            self.state.cached.insert(name.to_string());
        } else {
            self.state.cached.remove(name);
        }
    }
}

impl<'ast> Visit<'ast> for FunctionFlow {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        // Evaluate RHS before binding/shadowing the LHS: the old binding
        // remains visible to the initializer, as in Rust.
        let value = node.init.as_ref().map(|init| init.expr.as_ref());
        if let Some(rhs) = value {
            self.visit_expr(rhs);
        }
        if let Some(name) = binding_name(&node.pat) {
            if let Some(rhs) = value {
                self.assign(&name, rhs);
                if client_constructor(rhs) {
                    self.state.clients.insert(name);
                } else {
                    self.state.clients.remove(&name);
                }
            } else {
                self.state.cached.remove(&name);
                self.state.clients.remove(&name);
            }
        }
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        self.visit_expr(&node.right);
        if let Expr::Path(path) = node.left.as_ref() {
            if path.qself.is_none() && path.path.segments.len() == 1 {
                let name = path.path.segments[0].ident.to_string();
                self.assign(&name, &node.right);
                self.state.clients.remove(&name);
            }
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let method = node.method.to_string();
        let cross_contract = matches!(method.as_str(), "invoke_contract" | "try_invoke_contract")
            || (is_client_receiver(&node.receiver, &self.state.clients)
                && !matches!(method.as_str(), "new" | "clone" | "address" | "clone_from"));
        visit::visit_expr_method_call(self, node);
        if cross_contract {
            self.state.external_seen = true;
        }
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.decision(&node.cond, node.if_token.span.start().line);
        self.visit_expr(&node.cond);
        let entry = self.state.clone();

        self.visit_block(&node.then_branch);
        let then_flow = self.state.clone();

        self.state = entry.clone();
        if let Some((_, otherwise)) = &node.else_branch {
            self.visit_expr(otherwise);
        }
        self.state = FlowState::join(&then_flow, &self.state);
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        self.decision(&node.expr, node.match_token.span.start().line);
        self.visit_expr(&node.expr);
        let entry = self.state.clone();
        let mut joined: Option<FlowState> = None;
        for arm in &node.arms {
            self.state = entry.clone();
            if let Some((_, guard)) = &arm.guard {
                self.decision(guard, guard.span().start().line);
                self.visit_expr(guard);
            }
            self.visit_expr(&arm.body);
            joined = Some(match joined {
                Some(other) => FlowState::join(&other, &self.state),
                None => self.state.clone(),
            });
        }
        self.state = joined.unwrap_or(entry);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.decision(&node.cond, node.while_token.span.start().line);
        self.visit_expr(&node.cond);
        let entry = self.state.clone();
        self.visit_block(&node.body);
        self.state = FlowState::join(&entry, &self.state);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.decision(&node.expr, node.for_token.span.start().line);
        self.visit_expr(&node.expr);
        let entry = self.state.clone();
        self.visit_block(&node.body);
        self.state = FlowState::join(&entry, &self.state);
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        let entry = self.state.clone();
        self.visit_block(&node.body);
        self.state = FlowState::join(&entry, &self.state);
    }
}

fn binding_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        Pat::Type(typed) => binding_name(&typed.pat),
        _ => None,
    }
}

fn storage_receiver(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(call) => {
            matches!(
                call.method.to_string().as_str(),
                "persistent" | "temporary" | "instance"
            ) || storage_receiver(&call.receiver)
        }
        Expr::Reference(reference) => storage_receiver(&reference.expr),
        Expr::Paren(paren) => storage_receiver(&paren.expr),
        _ => false,
    }
}

fn has_storage_read(expr: &Expr) -> bool {
    struct StorageProbe(bool);
    impl<'ast> Visit<'ast> for StorageProbe {
        fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
            if matches!(node.method.to_string().as_str(), "get" | "has" | "contains_key")
                && storage_receiver(&node.receiver)
            {
                self.0 = true;
            }
            visit::visit_expr_method_call(self, node);
        }
    }
    let mut probe = StorageProbe(false);
    probe.visit_expr(expr);
    probe.0
}

fn used_names(expr: &Expr) -> HashSet<String> {
    #[derive(Default)]
    struct Names(HashSet<String>);
    impl<'ast> Visit<'ast> for Names {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if node.qself.is_none() && node.path.segments.len() == 1 {
                self.0.insert(node.path.segments[0].ident.to_string());
            }
            visit::visit_expr_path(self, node);
        }
    }
    let mut used = Names::default();
    used.visit_expr(expr);
    used.0
}

fn client_constructor(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    let Expr::Path(function) = call.func.as_ref() else {
        return false;
    };
    let parts: Vec<_> = function.path.segments.iter().collect();
    parts.len() > 1
        && parts[parts.len() - 1].ident == "new"
        && parts[parts.len() - 2].ident.to_string().ends_with("Client")
}

fn is_client_receiver(expr: &Expr, clients: &HashSet<String>) -> bool {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            clients.contains(&path.path.segments[0].ident.to_string())
        }
        Expr::Call(_) => client_constructor(expr),
        Expr::MethodCall(call) => is_client_receiver(&call.receiver, clients),
        Expr::Reference(reference) => is_client_receiver(&reference.expr, clients),
        Expr::Paren(paren) => is_client_receiver(&paren.expr, clients),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_stale_before_external_call() {
        let source = r#"
            impl Vault {
                pub fn withdraw(env: Env, remote: Address) {
                    let balance: i128 = env.storage().persistent().get(&Key::Balance).unwrap();
                    env.invoke_contract::<()>(&remote, &symbol_short!("update"), vec![&env]);
                    if balance > 0 { return; }
                }
            }
        "#;
        let findings = StaleStatePostcallRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        insta::assert_snapshot!(findings[0].message, @"SANCT_STALE_STATE_POSTCALL: withdraw uses cached storage variable balance in a decision after an external call");
    }

    #[test]
    fn fresh_reassignment_after_call_is_not_stale() {
        let source = r#"
            impl Vault {
                pub fn withdraw(env: Env, remote: Address) {
                    let mut balance = env.storage().persistent().get(&Key::Balance).unwrap();
                    env.invoke_contract::<()>(&remote, &symbol_short!("update"), vec![&env]);
                    balance = env.storage().persistent().get(&Key::Balance).unwrap();
                    if balance > 0 { return; }
                }
            }
        "#;
        assert!(StaleStatePostcallRule::new().check(source).is_empty());
    }

    #[test]
    fn fresh_shadowed_value_after_call_is_not_stale() {
        let source = r#"
            fn run(env: Env, remote: Address) {
                let balance = env.storage().instance().get(&Key::Balance).unwrap();
                env.invoke_contract::<()>(&remote, &symbol_short!("update"), vec![&env]);
                let balance = env.storage().instance().get(&Key::Balance).unwrap();
                match balance { 0 => {}, _ => {} }
            }
        "#;
        assert!(StaleStatePostcallRule::new().check(source).is_empty());
    }

    #[test]
    fn tracks_generated_client_external_calls() {
        let source = r#"
            impl Vault {
                pub fn transfer(env: Env, remote: Address) {
                    let client = TokenClient::new(&env, &remote);
                    let amount: i128 = env.storage().persistent().get(&Key::Amount).unwrap();
                    client.transfer(&env.current_contract_address(), &remote, &1);
                    match amount { 0 => {}, _ => {} }
                }
            }
        "#;
        let findings = StaleStatePostcallRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn direct_post_call_storage_get_is_fresh() {
        let source = r#"
            fn run(env: Env, remote: Address) {
                env.invoke_contract::<()>(&remote, &symbol_short!("update"), vec![&env]);
                if env.storage().persistent().get(&Key::Balance).unwrap_or(0) > 0 { return; }
            }
        "#;
        assert!(StaleStatePostcallRule::new().check(source).is_empty());
    }

    #[test]
    fn pre_call_decision_and_plain_mutation_do_not_trigger() {
        let source = r#"
            fn run(env: Env, remote: Address) {
                let balance = env.storage().instance().get(&Key::Balance).unwrap();
                if balance > 0 { return; }
                env.storage().instance().set(&Key::Ready, &true);
            }
        "#;
        assert!(StaleStatePostcallRule::new().check(source).is_empty());
    }
}
