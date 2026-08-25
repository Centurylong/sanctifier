use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::BTreeSet;
use syn::visit::Visit;

const FINDING_CODE: &str = "SANCT_AUTH_ON_CALLER";

/// Detects entrypoints that authorize the transaction caller while mutating
/// state owned by a *different* address parameter.
///
/// This is the confused-deputy shape. `caller.require_auth()` proves only that
/// the caller signed; if the state being written is keyed by `from`, the caller
/// has authorized a change to somebody else's balance. The contract dutifully
/// carries out an action on behalf of a principal that never consented — the
/// authorization is real, it is just attached to the wrong address.
///
/// The rule is deliberately structural rather than name-based: it compares the
/// set of address parameters that are authorized against the set used as the
/// owner key of a storage write. Naming is used only to phrase the message, so
/// a contract that calls its parameters `a` and `b` is still analysed.
pub struct AuthOnCallerRule;

impl AuthOnCallerRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        fn_name: &str,
        visibility: &syn::Visibility,
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        if !matches!(visibility, syn::Visibility::Public(_)) {
            return Vec::new();
        }

        let addresses = address_params(sig);
        // With a single address parameter there is no "other" owner to confuse
        // it with; an entrypoint missing auth entirely is `auth_gap`'s job.
        if addresses.len() < 2 {
            return Vec::new();
        }

        let mut visitor = AuthOwnerVisitor {
            addresses: &addresses,
            authorized: BTreeSet::new(),
            written_owners: Vec::new(),
        };
        visitor.visit_block(block);

        // No authorization at all is a different finding, reported elsewhere.
        // This rule is specifically about auth that is present but misplaced.
        if visitor.authorized.is_empty() {
            return Vec::new();
        }

        let authorized = visitor.authorized;
        let mut reported: BTreeSet<String> = BTreeSet::new();

        visitor
            .written_owners
            .into_iter()
            // Correct owner-auth: the owner whose state changes signed for it.
            .filter(|write| !authorized.contains(&write.owner))
            .filter(|write| reported.insert(write.owner.clone()))
            .map(|write| {
                let signer = authorized
                    .iter()
                    .next()
                    .cloned()
                    .unwrap_or_else(|| "the caller".to_string());
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Error,
                    format!(
                        "{FINDING_CODE}: `{signer}` is authorized but state owned by `{}` is mutated",
                        write.owner
                    ),
                    format!("{fn_name}:{}", write.line),
                )
                .with_suggestion(format!(
                    "Call `{0}.require_auth()` — the address whose state changes must be the one \
                     that authorizes the change. If `{signer}` is acting on `{0}`'s behalf, check a \
                     recorded allowance from `{0}` as well.",
                    write.owner
                ))
            })
            .collect()
    }
}

impl Default for AuthOnCallerRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for AuthOnCallerRule {
    fn name(&self) -> &str {
        "auth_on_caller"
    }

    fn description(&self) -> &str {
        "Detects require_auth on the caller while state belonging to a different address parameter is mutated."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(f) => (*f).clone(),
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

/// One storage write, and the address parameter that keys it.
struct OwnerWrite {
    owner: String,
    line: usize,
}

struct FunctionVisitor<'a> {
    rule: &'a AuthOnCallerRule,
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
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_impl_item_fn(self, node);
    }
}

struct AuthOwnerVisitor<'a> {
    addresses: &'a BTreeSet<String>,
    authorized: BTreeSet<String>,
    written_owners: Vec<OwnerWrite>,
}

impl<'ast> Visit<'ast> for AuthOwnerVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if method == "require_auth" || method == "require_auth_for_args" {
            if let Some(name) = receiver_ident(&node.receiver) {
                if self.addresses.contains(&name) {
                    self.authorized.insert(name);
                }
            }
        }

        // A storage write: `env.storage().<durability>().set(&key, &value)`.
        // The owner is whichever address parameter appears in the key.
        if matches!(method.as_str(), "set" | "remove" | "update")
            && chain_mentions_storage(&node.receiver)
        {
            if let Some(key) = node.args.first() {
                let mut idents = IdentCollector {
                    addresses: self.addresses,
                    found: BTreeSet::new(),
                };
                idents.visit_expr(key);
                for owner in idents.found {
                    self.written_owners.push(OwnerWrite {
                        owner,
                        line: line_of(node),
                    });
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Collects address-parameter identifiers mentioned anywhere in an expression.
struct IdentCollector<'a> {
    addresses: &'a BTreeSet<String>,
    found: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for IdentCollector<'_> {
    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        let name = node.to_string();
        if self.addresses.contains(&name) {
            self.found.insert(name);
        }
    }
}

/// Parameter names whose declared type mentions `Address`.
fn address_params(sig: &syn::Signature) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for input in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(ident) = &*pat_type.pat {
                if type_mentions_address(&pat_type.ty) {
                    out.insert(ident.ident.to_string());
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
            .is_some_and(|seg| seg.ident == "Address"),
        syn::Type::Reference(reference) => type_mentions_address(&reference.elem),
        _ => false,
    }
}

/// True when a method-call receiver chain passes through `storage()`, which is
/// what distinguishes a persisted write from an ordinary local `set`.
fn chain_mentions_storage(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(call) => {
            call.method == "storage" || chain_mentions_storage(&call.receiver)
        }
        syn::Expr::Field(field) => chain_mentions_storage(&field.base),
        _ => false,
    }
}

fn receiver_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(|i| i.to_string()),
        syn::Expr::Reference(reference) => receiver_ident(&reference.expr),
        syn::Expr::MethodCall(call) if call.method == "clone" => receiver_ident(&call.receiver),
        _ => None,
    }
}

fn line_of(node: &syn::ExprMethodCall) -> usize {
    use syn::spanned::Spanned;
    node.span().start().line
}
