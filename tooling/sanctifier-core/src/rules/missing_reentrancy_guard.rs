use crate::finding_codes::MISSING_REENTRANCY_GUARD;
use crate::rules::{Rule, RuleViolation, Severity};
use quote::ToTokens;
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Advises on unguarded value-moving entrypoints that interact with other contracts.
///
/// This is intentionally intra-procedural: it requires an observable external
/// call, not merely a suspicious function name. A guarded transfer, a function
/// that only changes local storage, and a read-only cross-contract query are
/// not findings. The separate CEI detector examines ordering of state effects.
pub struct MissingReentrancyGuardRule;

impl MissingReentrancyGuardRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MissingReentrancyGuardRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for MissingReentrancyGuardRule {
    fn name(&self) -> &str {
        "missing_reentrancy_guard"
    }

    fn description(&self) -> &str {
        "Finds value-moving contract entrypoints that make external calls without an active reentrancy guard"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Some(file) = crate::parse_cache::parse_cached(source) else {
            return Vec::new();
        };
        let mut visitor = ContractVisitor::default();
        visitor.visit_file(&file);
        visitor.findings
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct ContractVisitor {
    test_depth: usize,
    findings: Vec<RuleViolation>,
}

impl ContractVisitor {
    fn inspect(&mut self, name: &str, line: usize, signature: &syn::Signature, block: &syn::Block) {
        let mut facts = EntryFacts::default();
        // Soroban clients are often supplied by callers rather than constructed
        // inside the entrypoint. Track typed parameters just like local clients.
        for input in &signature.inputs {
            if let syn::FnArg::Typed(arg) = input {
                if is_client_type(&arg.ty) {
                    if let syn::Pat::Ident(ident) = &*arg.pat {
                        facts.client_vars.insert(ident.ident.to_string());
                    }
                }
            }
        }
        facts.visit_block(block);
        if !(is_value_moving(name) || facts.has_value_move) {
            return;
        }
        if let Some(call_line) = facts.first_unguarded_external {
            self.findings.push(
                RuleViolation::new(
                    MISSING_REENTRANCY_GUARD,
                    Severity::Warning,
                    format!(
                        "{MISSING_REENTRANCY_GUARD}: value-moving entrypoint `{name}` makes a cross-contract call without an active reentrancy guard"
                    ),
                    format!("{name}:{call_line}"),
                )
                .with_suggestion(
                    "Acquire a SanctifiedGuard or ReentrancyGuard before the external call and keep it active until state transitions finish; also consider checks-effects-interactions".into(),
                ),
            );
        }
        let _ = line; // source location is the actionable external call, not the signature
    }
}

impl<'ast> Visit<'ast> for ContractVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let is_test = test_cfg(&node.attrs);
        if is_test {
            self.test_depth += 1;
        }
        syn::visit::visit_item_mod(self, node);
        if is_test {
            self.test_depth -= 1;
        }
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if self.test_depth == 0
            && !test_cfg(&node.attrs)
            && matches!(node.vis, syn::Visibility::Public(_))
        {
            self.inspect(
                &node.sig.ident.to_string(),
                node.sig.ident.span().start().line,
                &node.sig,
                &node.block,
            );
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if self.test_depth == 0
            && !test_cfg(&node.attrs)
            && matches!(node.vis, syn::Visibility::Public(_))
        {
            self.inspect(
                &node.sig.ident.to_string(),
                node.sig.ident.span().start().line,
                &node.sig,
                &node.block,
            );
        }
    }
}

fn test_cfg(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && {
                let meta = attr.meta.to_token_stream().to_string().replace(' ', "");
                meta.contains("(test)") && !meta.contains("not(test)")
            }
    })
}

fn is_value_moving(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "transfer", "withdraw", "redeem", "payout", "send", "mint",
        "burn", "swap", "drain", "claim", "unstake", "liquidate",
    ]
    .iter()
    .any(|word| lower.contains(word))
}

fn moves_value(method: &str) -> bool {
    ["transfer", "transfer_from", "mint", "burn", "swap", "send", "payout"]
        .iter()
        .any(|word| method == *word || method.starts_with(&format!("{word}_")))
}

#[derive(Default)]
struct EntryFacts {
    guard_vars: HashSet<String>,
    client_vars: HashSet<String>,
    guard_active: bool,
    first_unguarded_external: Option<usize>,
    has_value_move: bool,
}

impl EntryFacts {
    fn is_guard(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Path(path) => path.path.get_ident().is_some_and(|id| {
                self.guard_vars.contains(&id.to_string())
            }),
            syn::Expr::Reference(r) => self.is_guard(&r.expr),
            syn::Expr::Paren(p) => self.is_guard(&p.expr),
            syn::Expr::MethodCall(m) => self.is_guard(&m.receiver),
            _ => is_guard_type(expr),
        }
    }

    fn is_client(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Path(path) => path.path.get_ident().is_some_and(|id| {
                self.client_vars.contains(&id.to_string())
            }),
            syn::Expr::Reference(r) => self.is_client(&r.expr),
            syn::Expr::Paren(p) => self.is_client(&p.expr),
            syn::Expr::MethodCall(m) => self.is_client(&m.receiver),
            _ => is_client_constructor(expr),
        }
    }
}

fn is_guard_type(expr: &syn::Expr) -> bool {
    let tokens = expr.to_token_stream().to_string();
    tokens.contains("SanctifiedGuard") || tokens.contains("ReentrancyGuard")
}

fn is_client_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident.to_string().ends_with("Client")),
        syn::Type::Reference(reference) => is_client_type(&reference.elem),
        syn::Type::Paren(paren) => is_client_type(&paren.elem),
        syn::Type::Group(group) => is_client_type(&group.elem),
        _ => false,
    }
}

fn is_client_constructor(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => match &*call.func {
            syn::Expr::Path(path) => {
                path.path.segments.iter().any(|s| s.ident.to_string().ends_with("Client"))
            }
            _ => false,
        },
        _ => false,
    }
}

impl<'ast> Visit<'ast> for EntryFacts {
    fn visit_stmt(&mut self, node: &'ast syn::Stmt) {
        // An item statement declares a local helper/type; it does not execute
        // that helper's body as part of the surrounding entrypoint.
        if matches!(node, syn::Stmt::Item(_)) {
            return;
        }
        syn::visit::visit_stmt(self, node);
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        let active_on_entry = self.guard_active;
        let guards_on_entry = self.guard_vars.clone();
        let clients_on_entry = self.client_vars.clone();
        syn::visit::visit_block(self, node);
        // An RAII guard acquired inside a nested lexical block has dropped
        // by the time the surrounding block resumes. A pre-existing guard
        // survives only if it was not released inside that block.
        self.guard_active &= active_on_entry;
        self.guard_vars = guards_on_entry;
        self.client_vars = clients_on_entry;
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.visit_expr(&node.cond);
        let active_before = self.guard_active;
        self.visit_block(&node.then_branch);
        let active_after_then = self.guard_active;

        // The alternative starts from the same pre-branch guard state.
        self.guard_active = active_before;
        if let Some((_, alternate)) = &node.else_branch {
            self.visit_expr(alternate);
        }
        // Only retain a guard after if/else when every possible path
        // retains it; one optional acquisition cannot guard later calls.
        self.guard_active &= active_after_then;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let (syn::Pat::Ident(ident), Some(init)) = (&node.pat, &node.init) {
            let name = ident.ident.to_string();
            if is_guard_type(&init.expr) {
                self.guard_vars.insert(name.clone());
            }
            if is_client_constructor(&init.expr) {
                self.client_vars.insert(name);
            }
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let segments: Vec<_> = path.path.segments.iter().map(|s| s.ident.to_string()).collect();
            let guard = segments.iter().any(|s| s == "SanctifiedGuard" || s == "ReentrancyGuard");
            let action = segments.last().map(String::as_str).unwrap_or("");
            // Explicitly dropping the sole tracked RAII guard ends its
            // protection before any later token transfer or external call.
            // Do not treat drop(&guard) (only a reference) as destruction.
            // Multiple live guards remain conservative without alias analysis.
            if matches!(
                segments.join("::").as_str(),
                "drop" | "std::mem::drop" | "core::mem::drop"
            ) && node.args.len() == 1 && self.guard_vars.len() == 1 {
                if let Some(syn::Expr::Path(dropped)) = node.args.first() {
                    if let Some(ident) = dropped.path.get_ident() {
                        if self.guard_vars.remove(&ident.to_string()) {
                            self.guard_active = false;
                        }
                    }
                }
            }
            if guard && matches!(action, "enter" | "try_enter" | "acquire") {
                self.guard_active = true;
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if self.is_guard(&node.receiver) {
            if matches!(method.as_str(), "enter" | "try_enter" | "acquire") {
                self.guard_active = true;
            } else if matches!(method.as_str(), "exit" | "release" | "unlock") {
                self.guard_active = false;
            }
        }

        let client = self.is_client(&node.receiver);
        let external = method == "invoke_contract"
            || (client && !matches!(method.as_str(), "clone" | "address" | "new"));
        if client && moves_value(&method) {
            self.has_value_move = true;
        }
        if external && !self.guard_active && self.first_unguarded_external.is_none() {
            self.first_unguarded_external = Some(node.method.span().start().line);
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_flags_unguarded_token_transfer() {
        let src = r#"
            impl Vault {
                pub fn withdraw(env: Env, token: Address, to: Address, amount: i128) {
                    TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &to, &amount);
                }
            }
        "#;
        let findings = MissingReentrancyGuardRule::new().check(src);
        let report = findings
            .iter()
            .map(|f| format!("{} | {:?}", f.rule_name, f.severity))
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!(report, @"SANCT_MISSING_REENTRANCY_GUARD | Warning");
        assert_eq!(findings.len(), 1);
        assert!(findings[0].location.starts_with("withdraw:"));
    }

    #[test]
    fn respects_active_reentrancy_guard() {
        let src = r#"
            impl Vault {
                pub fn withdraw(env: Env, token: Address, amount: i128) {
                    let guard = ReentrancyGuard::new(&env);
                    guard.enter();
                    TokenClient::new(&env, &token).transfer(&to, &amount);
                    guard.exit();
                }
            }
        "#;
        assert!(MissingReentrancyGuardRule::new().check(src).is_empty());
    }

    #[test]
    fn recognizes_sanctified_guard_and_detects_early_release() {
        let secure = r#"
            impl Vault {
                pub fn drain(env: Env, target: Address) {
                    let _guard = SanctifiedGuard::enter(&env);
                    env.invoke_contract::<()>(&target, &symbol_short!("drain"), vec![&env]);
                }
            }
        "#;
        let unsafe_src = r#"
            impl Vault {
                pub fn drain(env: Env, target: Address) {
                    let g = SanctifiedGuard::new(&env);
                    g.enter();
                    g.exit();
                    env.invoke_contract::<()>(&target, &symbol_short!("drain"), vec![&env]);
                }
            }
        "#;
        let rule = MissingReentrancyGuardRule::new();
        assert!(rule.check(secure).is_empty());
        assert_eq!(rule.check(unsafe_src).len(), 1);
    }

    #[test]
    fn unused_local_helper_cannot_create_outer_reentrancy_finding() {
        let src = r#"
            impl Vault {
                pub fn inspect(env: Env, token: Address, to: Address, amount: i128) {
                    // This is a declaration, never called by inspect.
                    fn unused_transfer(env: Env, token: Address, to: Address, amount: i128) {
                        TokenClient::new(&env, &token).transfer(&to, &amount);
                    }
                    let nothing = 1;
                }
            }
        "#;
        assert!(MissingReentrancyGuardRule::new().check(src).is_empty());
    }

    #[test]
    fn explicitly_dropped_sole_guard_does_not_hide_value_transfer() {
        let guarded = r#"
            impl Vault {
                pub fn withdraw(env: Env, token: Address, to: Address, amount: i128) {
                    let guard = SanctifiedGuard::enter(&env);
                    TokenClient::new(&env, &token).transfer(&to, &amount);
                }
            }
        "#;
        assert!(MissingReentrancyGuardRule::new().check(guarded).is_empty());

        for drop_path in ["drop", "std::mem::drop", "core::mem::drop"] {
            let unguarded = guarded.replace(
                "TokenClient::new(&env, &token).transfer",
                &format!("{drop_path}(guard); TokenClient::new(&env, &token).transfer"),
            );
            let findings = MissingReentrancyGuardRule::new().check(&unguarded);
            assert_eq!(findings.len(), 1, "{drop_path} must release the guard");
            assert!(findings[0].location.starts_with("withdraw:"));
        }

        // Dropping just a reference does not destroy the RAII guard.
        let borrowed = guarded.replace(
            "TokenClient::new(&env, &token).transfer",
            "drop(&guard); TokenClient::new(&env, &token).transfer",
        );
        assert!(MissingReentrancyGuardRule::new().check(&borrowed).is_empty());
    }

    #[test]
    fn ignores_read_only_queries_and_local_only_transfers() {
        let src = r#"
            impl Vault {
                pub fn query_remote(env: Env, target: Address) {
                    env.invoke_contract::<()>(&target, &symbol_short!("balance"), vec![&env]);
                }
                pub fn withdraw_local(env: Env, key: DataKey) {
                    env.storage().persistent().set(&key, &0_i128);
                }
            }
        "#;
        assert!(MissingReentrancyGuardRule::new().check(src).is_empty());
    }

    #[test]
    fn detects_value_transfer_from_borrowed_client_parameters() {
        let src = r#"
            impl Vault {
                pub fn pay(client: &TokenClient, to: Address, amount: i128) {
                    client.transfer(&to, &amount);
                }
            }
        "#;
        let findings = MissingReentrancyGuardRule::new().check(src);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].location.starts_with("pay:"));

        // A similarly named method on a non-client parameter must not
        // manufacture a cross-contract call or value-movement finding.
        let non_client = r#"
            impl Vault {
                pub fn pay(store: &LocalStore, to: Address, amount: i128) {
                    store.transfer(&to, &amount);
                }
            }
        "#;
        assert!(MissingReentrancyGuardRule::new().check(non_client).is_empty());
    }

    #[test]
    fn handles_named_clients_and_excludes_test_modules() {
        let src = r#"
            impl Vault {
                pub fn execute(env: Env, token: Address, to: Address, amount: i128) {
                    let client = TokenClient::new(&env, &token);
                    client.transfer(&to, &amount);
                }
            }
            #[cfg(test)]
            mod tests {
                impl Mock {
                    pub fn withdraw(env: Env, t: Address) {
                        env.invoke_contract::<()>(&t, &sym, vec![&env]);
                    }
                }
            }
        "#;
        let findings = MissingReentrancyGuardRule::new().check(src);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].location.starts_with("execute:"));
    }

    #[test]
    fn optional_and_expired_guards_do_not_suppress_later_external_calls() {
        let source = r#"
impl Vault {
    pub fn withdraw_conditionally(env: Env, token: Address, to: Address, amount: i128, flag: bool) {
        if flag {
            let _guard = SanctifiedGuard::enter(&env);
        }
        TokenClient::new(&env, &token).transfer(&to, &amount);
    }
    pub fn withdraw_expired_guard(env: Env, token: Address, to: Address, amount: i128) {
        { let _guard = SanctifiedGuard::enter(&env); }
        TokenClient::new(&env, &token).transfer(&to, &amount);
    }
    pub fn withdraw_guarded(env: Env, token: Address, to: Address, amount: i128) {
        let _guard = SanctifiedGuard::enter(&env);
        TokenClient::new(&env, &token).transfer(&to, &amount);
    }
}
"#;
        let findings = MissingReentrancyGuardRule::new().check(source);
        let names: Vec<_> = findings
            .iter()
            .map(|f| f.location.split_once(':').unwrap().0)
            .collect();
        assert_eq!(
            names,
            vec!["withdraw_conditionally", "withdraw_expired_guard"],
            "an optional or already-dropped guard must not suppress the warning"
        );
    }
}
