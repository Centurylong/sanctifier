//! Detect mismatched authorization subjects when a storage-sourced Address
//! (not a second entrypoint parameter) is used as the owner of a state effect.
//!
//! Unlike auth_on_caller, this rule is useful with exactly one Address parameter.
//! It is a conservative, intraprocedural analysis: unknown subject provenance
//! is never treated as proof of a mismatch.
use crate::finding_codes::AUTH_SUBJECT;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{BTreeMap, BTreeSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

pub struct AuthSubjectRule;

impl AuthSubjectRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        name: &str,
        vis: &syn::Visibility,
        signature: &syn::Signature,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        if !matches!(vis, syn::Visibility::Public(_)) {
            return Vec::new();
        }

        let mut flow = SubjectFlow::default();
        for argument in &signature.inputs {
            if let syn::FnArg::Typed(arg) = argument {
                if is_address_type(&arg.ty) {
                    if let Some(name) = binding_name(&arg.pat) {
                        flow.subjects.insert(name.clone(), Subject::Argument(name));
                    }
                }
            }
        }
        flow.visit_block(block);
        if flow.authorized.is_empty() || flow.effects.is_empty() {
            return Vec::new();
        }

        // A normal transfer may credit an unauthenticated recipient; do
        // not flag its recipient just because a different party signs.
        // However, a debit/delete of a storage-sourced victim remains unsafe
        // even if the same function *also* credits the authenticated caller.
        // Evaluating just the union of effect owners would miss that theft.
        let harmful_mismatch = flow.effects.iter().find(|effect| {
            effect.owner_must_authorize
                && effect.owners.is_disjoint(&flow.authorized)
                && effect.owners.iter().any(|s| matches!(s, Subject::Loaded(_)))
        });

        let effect = if let Some(effect) = harmful_mismatch {
            effect
        } else {
            if flow.effects.iter().any(|effect| {
                !effect.owners.is_disjoint(&flow.authorized)
            }) {
                return Vec::new();
            }
            let Some(effect) = flow.effects.iter().find(|effect| {
                effect.owners.iter().any(|s| matches!(s, Subject::Loaded(_)))
            }) else {
                return Vec::new();
            };
            effect
        };
        let owner = effect.owners.iter().find(|s| matches!(s, Subject::Loaded(_))).unwrap();
        let line = effect.line;
        let signer = flow.authorized.iter().next().unwrap();

        vec![RuleViolation::new(
            AUTH_SUBJECT,
            Severity::Error,
            format!(
                "{AUTH_SUBJECT}: authorization of {} does not bind the state effect on {}; the effect owner came from storage",
                signer.label(),
                owner.label()
            ),
            format!("{name}:{line}"),
        )
        .with_suggestion(
            "Require authorization from the actual state owner, or verify a recorded allowance binding the authorized actor to that owner before the debit/write.".into(),
        )]
    }
}

impl Default for AuthSubjectRule {
    fn default() -> Self { Self::new() }
}

impl Rule for AuthSubjectRule {
    fn name(&self) -> &str { "auth_subject" }
    fn description(&self) -> &str {
        "Detects public state effects on a storage-loaded Address when another subject is authorized."
    }
    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Some(parsed) = crate::parse_cache::parse_cached(source) else {
            return Vec::new();
        };
        let mut visitor = FunctionVisitor { rule: self, findings: Vec::new() };
        visitor.visit_file(&parsed);
        visitor.findings
    }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Subject {
    Argument(String),
    Loaded(String),
}

impl Subject {
    fn label(&self) -> &str {
        match self {
            Subject::Argument(name) | Subject::Loaded(name) => name,
        }
    }
}

struct FunctionVisitor<'r> {
    rule: &'r AuthSubjectRule,
    findings: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FunctionVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.findings.extend(self.rule.check_function(
            &node.sig.ident.to_string(), &node.vis, &node.sig, &node.block,
        ));
        visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.findings.extend(self.rule.check_function(
            &node.sig.ident.to_string(), &node.vis, &node.sig, &node.block,
        ));
        visit::visit_impl_item_fn(self, node);
    }
}

struct OwnerEffect {
    owners: BTreeSet<Subject>,
    line: usize,
    // A balance debit/burn/withdrawal or storage removal requires a
    // distinct owner consent regardless of unrelated credited balances.
    owner_must_authorize: bool,
}

#[derive(Default)]
struct SubjectFlow {
    // Aliases retain the origin (entrypoint argument or storage-loaded Address).
    subjects: BTreeMap<String, Subject>,
    storage_handles: BTreeSet<String>,
    authorized: BTreeSet<Subject>,
    effects: Vec<OwnerEffect>,
}

impl SubjectFlow {
    fn origin(&self, expr: &syn::Expr) -> Option<Subject> {
        let name = ident_name(expr)?;
        self.subjects.get(&name).cloned()
    }

    fn owners_in(&self, expr: &syn::Expr) -> BTreeSet<Subject> {
        let mut collector = Owners { subjects: &self.subjects, found: BTreeSet::new() };
        collector.visit_expr(expr);
        collector.found
    }

    fn is_storage_chain(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::MethodCall(call) => {
                call.method == "storage" || self.is_storage_chain(&call.receiver)
            }
            syn::Expr::Field(field) => self.is_storage_chain(&field.base),
            syn::Expr::Reference(reference) => self.is_storage_chain(&reference.expr),
            syn::Expr::Paren(paren) => self.is_storage_chain(&paren.expr),
            syn::Expr::Path(path) => path.path.get_ident()
                .is_some_and(|id| self.storage_handles.contains(&id.to_string())),
            _ => false,
        }
    }

    fn is_storage_load(&self, expr: &syn::Expr) -> bool {
        struct Reads<'f> { flow: &'f SubjectFlow, found: bool }
        impl<'ast> Visit<'ast> for Reads<'_> {
            fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
                if call.method == "get" && self.flow.is_storage_chain(&call.receiver) {
                    self.found = true;
                }
                visit::visit_expr_method_call(self, call);
            }
        }
        let mut reads = Reads { flow: self, found: false };
        reads.visit_expr(expr);
        reads.found
    }
}

impl<'ast> Visit<'ast> for SubjectFlow {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            // Traverse initializer with existing aliases before rebinding.
            self.visit_expr(&init.expr);
        }
        let Some(name) = binding_name(&local.pat) else { return };
        let init = local.init.as_ref().map(|i| &*i.expr);
        if init.is_some_and(|i| self.is_storage_chain(i)) {
            self.storage_handles.insert(name.clone());
        } else {
            self.storage_handles.remove(&name);
        }

        let new_origin = init.and_then(|expr| {
            self.origin(expr).or_else(|| {
                self.is_storage_load(expr).then(|| Subject::Loaded(name.clone()))
            })
        });
        if let Some(origin) = new_origin {
            self.subjects.insert(name, origin);
        } else {
            self.subjects.remove(&name);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if matches!(call.method.to_string().as_str(), "require_auth" | "require_auth_for_args") {
            if let Some(origin) = self.origin(&call.receiver) {
                self.authorized.insert(origin);
            }
        }

        if matches!(call.method.to_string().as_str(), "set" | "remove" | "update")
            && self.is_storage_chain(&call.receiver)
        {
            if let Some(key) = call.args.first() {
                let owners = self.owners_in(key);
                if !owners.is_empty() {
                    self.effects.push(OwnerEffect {
                        owners,
                        line: call.span().start().line,
                        owner_must_authorize: call.method == "remove",
                    });
                }
            }
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let helper = match &*call.func {
            syn::Expr::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
            _ => None,
        };
        // Qualified local balance mutators: no attempt to guess what arbitrary
        // helper calls do. A benign accessor with an Address argument is silent.
        if helper.as_deref().is_some_and(|name| matches!(
            name, "debit" | "withdraw" | "debit_balance" | "burn_from" | "spend_from" | "set_balance"
        )) {
            let owners = call.args.iter().flat_map(|arg| self.owners_in(arg)).collect();
            if !owners.is_empty() {
                self.effects.push(OwnerEffect {
                    owners,
                    line: call.span().start().line,
                    owner_must_authorize: helper.as_deref() != Some("set_balance"),
                });
            }
        }
        visit::visit_expr_call(self, call);
    }
}

struct Owners<'s> {
    subjects: &'s BTreeMap<String, Subject>,
    found: BTreeSet<Subject>,
}

impl<'ast> Visit<'ast> for Owners<'_> {
    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(name) = path.path.get_ident() {
            if let Some(origin) = self.subjects.get(&name.to_string()) {
                self.found.insert(origin.clone());
            }
        }
        visit::visit_expr_path(self, path);
    }
}

fn is_address_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.last()
            .is_some_and(|s| s.ident == "Address"),
        syn::Type::Reference(reference) => is_address_type(&reference.elem),
        _ => false,
    }
}

fn binding_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(id) => Some(id.ident.to_string()),
        syn::Pat::Type(typed) => binding_name(&typed.pat),
        _ => None,
    }
}

fn ident_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(|id| id.to_string()),
        syn::Expr::Reference(reference) => ident_name(&reference.expr),
        syn::Expr::Paren(paren) => ident_name(&paren.expr),
        syn::Expr::MethodCall(call) if call.method == "clone" => ident_name(&call.receiver),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_storage_victim_through_alias_and_helper() {
        let source = r#"
            impl Vault {
                pub fn withdraw(e: Env, from: Address, amount: i128) {
                    from.require_auth();
                    let victim: Address = e.storage().persistent().get(&0).unwrap();
                    let owner = victim.clone();
                    debit(&e, &owner, amount);
                }
            }
        "#;
        let hits = AuthSubjectRule::new().check(source);
        assert_eq!(hits.len(), 1, "{hits:#?}");
        assert_eq!(hits[0].rule_name, AUTH_SUBJECT);
        assert!(hits[0].message.contains("victim"));
    }

    #[test]
    fn finds_direct_state_write_through_storage_alias() {
        let source = r#"
            impl Vault {
                pub fn sweep(e: Env, signer: Address) {
                    signer.require_auth();
                    let storage = e.storage().persistent();
                    let victim = storage.get(&0).unwrap();
                    storage.set(&victim, &0i128);
                }
            }
        "#;
        assert_eq!(AuthSubjectRule::new().check(source).len(), 1);
    }

    #[test]
    fn accepts_auth_of_storage_subject_and_normal_transfer() {
        let source = r#"
            impl Vault {
                pub fn correct(e: Env, from: Address) {
                    let victim = e.storage().persistent().get(&0).unwrap();
                    victim.require_auth();
                    debit(&e, &victim, 1);
                }
                pub fn normal(e: Env, from: Address, to: Address) {
                    from.require_auth();
                    e.storage().persistent().set(&from, &0);
                    e.storage().persistent().set(&to, &1);
                }
            }
        "#;
        assert!(AuthSubjectRule::new().check(source).is_empty());
    }

    #[test]
    fn reports_victim_debit_even_when_attacker_gets_a_credit() {
        let source = r#"
            impl Vault {
                pub fn siphon(e: Env, attacker: Address, amount: i128) {
                    attacker.require_auth();
                    let victim: Address = e.storage().persistent().get(&7).unwrap();
                    debit(&e, &victim, amount);
                    e.storage().persistent().set(&attacker, &amount);
                }
            }
        "#;
        let hits = AuthSubjectRule::new().check(source);
        assert_eq!(hits.len(), 1, "{hits:#?}");
        assert!(hits[0].message.contains("victim"));
    }

    #[test]
    fn ignores_signed_sender_and_storage_loaded_recipient_credit() {
        let source = r#"
            impl Vault {
                pub fn reward(e: Env, sender: Address) {
                    sender.require_auth();
                    let recipient: Address = e.storage().persistent().get(&7).unwrap();
                    e.storage().persistent().set(&sender, &0i128);
                    e.storage().persistent().set(&recipient, &10i128);
                }
            }
        "#;
        assert!(AuthSubjectRule::new().check(source).is_empty());
    }

    #[test]
    fn unknown_provenance_and_private_helpers_do_not_raise_findings() {
        let source = r#"
            impl Vault {
                fn helper(e: Env, from: Address) {
                    from.require_auth();
                    let unknown = other_source();
                    debit(&e, &unknown, 1);
                }
                pub fn view(e: Env, from: Address) {
                    from.require_auth();
                    let victim = e.storage().persistent().get(&0).unwrap();
                    let _ = victim;
                }
            }
        "#;
        assert!(AuthSubjectRule::new().check(source).is_empty());
    }
}
