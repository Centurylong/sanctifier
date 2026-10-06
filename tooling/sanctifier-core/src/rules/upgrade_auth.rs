use crate::finding_codes::UPGRADE_RISK;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::BTreeSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

const FINDING_CODE: &str = UPGRADE_RISK;

/// Detects public contract-upgrade paths that reach the Soroban WASM upgrade
/// primitive without both an administrator-shaped authorization guard and
/// nonce-shaped replay protection.
pub struct UpgradeAuthRule;

impl UpgradeAuthRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        name: &str,
        visibility: &syn::Visibility,
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        if !matches!(visibility, syn::Visibility::Public(_)) {
            return Vec::new();
        }

        let mut guard = UpgradeGuardVisitor {
            admin_addresses: admin_address_params(sig),
            has_upgrade_call: false,
            has_admin_auth: false,
            has_nonce_validation: false,
            has_nonce_consumption: false,
            nonce_bindings: BTreeSet::new(),
        };
        guard.visit_block(block);

        if !guard.has_upgrade_call {
            return Vec::new();
        }

        let has_nonce_guard = guard.has_nonce_validation && guard.has_nonce_consumption;
        if guard.has_admin_auth && has_nonce_guard {
            return Vec::new();
        }

        let missing = match (guard.has_admin_auth, has_nonce_guard) {
            (false, false) => "administrator authorization and nonce replay protection",
            (false, true) => "administrator authorization",
            (true, false) => "nonce replay protection",
            (true, true) => unreachable!(),
        };

        vec![RuleViolation::new(
            FINDING_CODE,
            Severity::Error,
            format!(
                "{FINDING_CODE}: public upgrade path calls `update_current_contract_wasm` without {missing}"
            ),
            format!("{}:{}", name, sig.span().start().line),
        )
        .with_suggestion(
            "Authenticate the current administrator and validate/consume a nonce before calling the contract WASM upgrade primitive."
                .to_string(),
        )]
    }
}

impl Default for UpgradeAuthRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UpgradeAuthRule {
    fn name(&self) -> &str {
        "upgrade_auth"
    }

    fn description(&self) -> &str {
        "Detects public contract-upgrade paths missing administrator authorization or nonce replay protection."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return vec![],
        };

        let mut visitor = FunctionVisitor {
            rule: self,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FunctionVisitor<'a> {
    rule: &'a UpgradeAuthRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FunctionVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        visit::visit_impl_item_fn(self, node);
    }
}

struct UpgradeGuardVisitor {
    admin_addresses: BTreeSet<String>,
    has_upgrade_call: bool,
    has_admin_auth: bool,
    has_nonce_validation: bool,
    has_nonce_consumption: bool,
    nonce_bindings: BTreeSet<String>,
}

fn is_admin_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("admin")
        || lower.contains("owner")
        || lower.contains("authority")
        || lower.contains("govern")
}

fn is_nonce_marker(value: &str) -> bool {
    value.to_ascii_lowercase().contains("nonce")
}

fn admin_address_params(sig: &syn::Signature) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for input in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(ident) = &*pat_type.pat {
                let name = ident.ident.to_string();
                if is_admin_marker(&name) && type_mentions_address(&pat_type.ty) {
                    out.insert(name);
                }
            }
        }
    }
    out
}

fn type_mentions_address(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Address"),
        syn::Type::Reference(reference) => type_mentions_address(&reference.elem),
        _ => false,
    }
}

fn receiver_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(|ident| ident.to_string()),
        syn::Expr::Reference(reference) => receiver_ident(&reference.expr),
        syn::Expr::MethodCall(call) if call.method == "clone" => receiver_ident(&call.receiver),
        _ => None,
    }
}

#[derive(Default)]
struct MarkerVisitor<'a> {
    has_nonce: bool,
    nonce_bindings: Option<&'a BTreeSet<String>>,
}

impl<'ast> Visit<'ast> for MarkerVisitor<'_> {
    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        let name = node.to_string();
        self.has_nonce |= is_nonce_marker(&name)
            || self
                .nonce_bindings
                .is_some_and(|bindings| bindings.contains(&name));
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.has_nonce |= is_nonce_marker(&node.value());
    }
}

fn expr_has_nonce_marker(expr: &syn::Expr) -> bool {
    let mut marker = MarkerVisitor::default();
    marker.visit_expr(expr);
    marker.has_nonce
}

fn expr_has_nonce_marker_or_binding(
    expr: &syn::Expr,
    nonce_bindings: &BTreeSet<String>,
) -> bool {
    let mut marker = MarkerVisitor {
        has_nonce: false,
        nonce_bindings: Some(nonce_bindings),
    };
    marker.visit_expr(expr);
    marker.has_nonce
}

fn local_binding_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Type(typed) => local_binding_ident(&typed.pat),
        _ => None,
    }
}

fn nonce_related_ident_count(
    tokens: proc_macro2::TokenStream,
    nonce_bindings: &BTreeSet<String>,
) -> usize {
    tokens
        .into_iter()
        .map(|token| match token {
            proc_macro2::TokenTree::Ident(ident) => {
                let name = ident.to_string();
                usize::from(is_nonce_marker(&name) || nonce_bindings.contains(&name))
            }
            proc_macro2::TokenTree::Group(group) => {
                nonce_related_ident_count(group.stream(), nonce_bindings)
            }
            _ => 0,
        })
        .sum()
}

impl<'ast> Visit<'ast> for UpgradeGuardVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let syn::Pat::Type(typed) = &node.pat {
            if let Some(name) = local_binding_ident(&node.pat) {
                if is_admin_marker(&name) && type_mentions_address(&typed.ty) {
                    self.admin_addresses.insert(name);
                }
            }
        }

        if let Some(init) = &node.init {
            if expr_has_nonce_marker_or_binding(&init.expr, &self.nonce_bindings) {
                if let Some(name) = local_binding_ident(&node.pat) {
                    self.nonce_bindings.insert(name);
                }
            }
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();

        if name == "update_current_contract_wasm" {
            self.has_upgrade_call = true;
        }

        if matches!(name.as_str(), "require_auth" | "require_auth_for_args") {
            if let Some(receiver) = receiver_ident(&node.receiver) {
                if self.admin_addresses.contains(&receiver) {
                    self.has_admin_auth = true;
                }
            }
        }

        if matches!(name.as_str(), "set" | "insert" | "put" | "replace")
            && (expr_has_nonce_marker(&node.receiver)
                || node.args.iter().any(expr_has_nonce_marker))
        {
            self.has_nonce_consumption = true;
        }

        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            let mut segments = path.path.segments.iter().rev();
            let name = segments.next().map(|segment| segment.ident.to_string());
            let receiver_type = segments.next().map(|segment| segment.ident.to_string());

            if name.as_deref() == Some("update_current_contract_wasm") {
                self.has_upgrade_call = true;
            }

            if name
                .as_deref()
                .is_some_and(|name| matches!(name, "require_auth" | "require_auth_for_args"))
                && receiver_type.as_deref() == Some("Address")
            {
                if let Some(first_arg) = node.args.first() {
                    if let Some(receiver) = receiver_ident(first_arg) {
                        if self.admin_addresses.contains(&receiver) {
                            self.has_admin_auth = true;
                        }
                    }
                }
            }
        }

        visit::visit_expr_call(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(
            &node.op,
            syn::BinOp::Eq(_)
                | syn::BinOp::Ne(_)
                | syn::BinOp::Lt(_)
                | syn::BinOp::Le(_)
                | syn::BinOp::Gt(_)
                | syn::BinOp::Ge(_)
        ) && expr_has_nonce_marker_or_binding(&node.left, &self.nonce_bindings)
            && expr_has_nonce_marker_or_binding(&node.right, &self.nonce_bindings)
        {
            self.has_nonce_validation = true;
        }

        visit::visit_expr_binary(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(segment) = node.path.segments.last() {
            let name = segment.ident.to_string();
            if matches!(
                name.as_str(),
                "assert" | "assert_eq" | "assert_ne" | "debug_assert" | "debug_assert_eq" | "debug_assert_ne"
            ) && nonce_related_ident_count(node.tokens.clone(), &self.nonce_bindings) >= 2
            {
                self.has_nonce_validation = true;
            }
        }

        visit::visit_macro(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshots_unguarded_upgrade() {
        let source = r#"
pub struct Contract;
impl Contract {
    pub fn upgrade(env: Env, wasm_hash: BytesN<32>) {
        env.deployer().update_current_contract_wasm(wasm_hash);
    }
}
"#;

        let findings = UpgradeAuthRule::new().check(source);
        assert_eq!(findings.len(), 1);
        let finding = &findings[0];
        let rendered = format!(
            "{}|{:?}|{}",
            finding.rule_name, finding.severity, finding.message
        );
        insta::assert_snapshot!(
            rendered,
            @"S010|Error|S010: public upgrade path calls `update_current_contract_wasm` without administrator authorization and nonce replay protection"
        );
    }

    #[test]
    fn accepts_admin_auth_plus_nonce_guard() {
        let source = r#"
pub struct Contract;
impl Contract {
    pub fn upgrade(
        env: Env,
        admin: Address,
        expected_nonce: u64,
        wasm_hash: BytesN<32>,
    ) {
        admin.require_auth_for_args((expected_nonce, wasm_hash.clone()));
        let stored_nonce: u64 = env.storage().instance().get(&"upgrade_nonce").unwrap_or(0);
        assert_eq!(stored_nonce, expected_nonce);
        env.storage().instance().set(&"upgrade_nonce", &(stored_nonce + 1));
        env.deployer().update_current_contract_wasm(wasm_hash);
    }
}
"#;

        assert!(UpgradeAuthRule::new().check(source).is_empty());
    }

    #[test]
    fn accepts_typed_storage_owner_plus_nonce_guard() {
        let source = r#"
pub fn upgrade(env: Env, nonce: u64, wasm_hash: BytesN<32>) {
    let owner: Address = env.storage().instance().get(&"OWNER").unwrap();
    owner.require_auth();
    let current_nonce: u64 = env.storage().instance().get(&"upgrade_nonce").unwrap_or(0);
    assert_eq!(current_nonce, nonce);
    env.storage().instance().set(&"upgrade_nonce", &(current_nonce + 1));
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        assert!(UpgradeAuthRule::new().check(source).is_empty());
    }

    #[test]
    fn accepts_nonce_derived_local_guard() {
        let source = r#"
pub fn upgrade(env: Env, admin: Address, nonce: u64, wasm_hash: BytesN<32>) {
    admin.require_auth_for_args((nonce, wasm_hash.clone()));
    let current: u64 = env.storage().instance().get(&"upgrade_nonce").unwrap_or(0);
    assert_eq!(current, nonce);
    env.storage().instance().set(&"upgrade_nonce", &(current + 1));
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        assert!(UpgradeAuthRule::new().check(source).is_empty());
    }

    #[test]
    fn accepts_address_ufcs_plus_nonce_guard() {
        let source = r#"
pub fn upgrade(
    env: Env,
    owner: Address,
    expected_nonce: u64,
    wasm_hash: BytesN<32>,
) {
    Address::require_auth(&owner);
    let stored_nonce: u64 = env.storage().instance().get(&"upgrade_nonce").unwrap_or(0);
    assert!(stored_nonce == expected_nonce);
    env.storage().instance().set(&"upgrade_nonce", &(stored_nonce + 1));
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        assert!(UpgradeAuthRule::new().check(source).is_empty());
    }

    #[test]
    fn rejects_name_only_guards_and_weak_nonce_check() {
        let source = r#"
macro_rules! check_admin { () => {}; }
macro_rules! validate_nonce { () => {}; }

struct FakeAdmin;
impl FakeAdmin { fn require_auth(&self) {} }

pub fn upgrade(
    env: Env,
    admin: FakeAdmin,
    nonce: u64,
    wasm_hash: BytesN<32>,
) {
    admin.require_auth();
    check_admin!();
    validate_nonce!();
    assert!(nonce > 0);
    env.storage().instance().set(&"upgrade_nonce", &nonce);
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        let findings = UpgradeAuthRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("administrator authorization"));
        assert!(findings[0].message.contains("nonce replay protection"));
    }

    #[test]
    fn flags_partial_guards() {
        let auth_only = r#"
pub fn upgrade(env: Env, admin: Address, wasm_hash: BytesN<32>) {
    admin.require_auth();
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;
        let nonce_only = r#"
pub fn upgrade(env: Env, nonce: u64, wasm_hash: BytesN<32>) {
    assert!(nonce > 0);
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        assert_eq!(UpgradeAuthRule::new().check(auth_only).len(), 1);
        assert_eq!(UpgradeAuthRule::new().check(nonce_only).len(), 1);
    }

    #[test]
    fn bare_nonce_name_does_not_count_as_replay_protection() {
        let source = r#"
pub fn upgrade(env: Env, admin: Address, nonce: u64, wasm_hash: BytesN<32>) {
    admin.require_auth();
    let _unused_nonce = nonce;
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        let findings = UpgradeAuthRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("nonce replay protection"));
    }

    #[test]
    fn unrelated_admin_name_does_not_bind_another_principals_auth() {
        let source = r#"
pub fn upgrade(
    env: Env,
    user: Address,
    admin_hint: Address,
    nonce: u64,
    wasm_hash: BytesN<32>,
) {
    user.require_auth();
    assert!(nonce > 0);
    let _ = admin_hint;
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        let findings = UpgradeAuthRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("administrator authorization"));
    }

    #[test]
    fn ignores_non_upgrade_and_private_helpers() {
        let source = r#"
pub fn configure(admin: Address) {
    admin.require_auth();
}
fn hidden_upgrade(env: Env, wasm_hash: BytesN<32>) {
    env.deployer().update_current_contract_wasm(wasm_hash);
}
"#;

        assert!(UpgradeAuthRule::new().check(source).is_empty());
    }
}