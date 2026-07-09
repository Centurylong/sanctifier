use crate::finding_codes::OWNERSHIP_TRANSFER;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects owner/admin handovers that directly replace control without a
/// propose/accept step.
pub struct OwnershipTransferRule;

impl OwnershipTransferRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OwnershipTransferRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for OwnershipTransferRule {
    fn name(&self) -> &str {
        "ownership_transfer"
    }

    fn description(&self) -> &str {
        "Detects one-step owner/admin transfers missing an accept step"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = OwnershipVisitor {
            violations: Vec::new(),
            suppressions: suppressions(source),
            test_depth: 0,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct OwnershipVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl OwnershipVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for OwnershipVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_test = has_cfg_test(&node.attrs);
        if was_test {
            self.test_depth += 1;
        }

        syn::visit::visit_item_mod(self, node);

        if was_test {
            self.test_depth -= 1;
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if self.in_test_module() || !has_attr(&node.attrs, "contractimpl") {
            syn::visit::visit_item_impl(self, node);
            return;
        }

        for item in &node.items {
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) {
                    continue;
                }

                if let Some(issue) = check_function(function) {
                    if !is_suppressed(&self.suppressions, issue.line) {
                        self.violations.push(issue.into_violation());
                    }
                }
            }
        }
    }
}

fn check_function(function: &syn::ImplItemFn) -> Option<OwnershipIssue> {
    let fn_name = function.sig.ident.to_string();
    if !is_owner_transfer_function(&fn_name) || is_two_step_function_name(&fn_name) {
        return None;
    }

    let params = sensitive_address_params(&function.sig);
    if params.is_empty() {
        return None;
    }

    let body = quote::quote!(#function.block).to_string();
    if body_uses_pending_owner(&body) || !body_writes_owner_slot(&body) {
        return None;
    }

    Some(OwnershipIssue {
        fn_name,
        param_name: params[0].name.clone(),
        line: params[0].line,
    })
}

#[derive(Debug, Clone)]
struct AddressParam {
    name: String,
    line: usize,
}

struct OwnershipIssue {
    fn_name: String,
    param_name: String,
    line: usize,
}

impl OwnershipIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            OWNERSHIP_TRANSFER,
            Severity::Warning,
            format!(
                "{OWNERSHIP_TRANSFER}: `{}` changes owner/admin in one step",
                self.fn_name
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(format!(
            "Store `{}` as a pending owner/admin and require the new address to call an accept function",
            self.param_name
        ))
    }
}

fn sensitive_address_params(sig: &syn::Signature) -> Vec<AddressParam> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) if is_address_type(&pat_type.ty) => {
                pat_ident(&pat_type.pat).and_then(|name| {
                    is_owner_param(&name).then_some(AddressParam {
                        name,
                        line: pat_type.span().start().line,
                    })
                })
            }
            _ => None,
        })
        .collect()
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

fn is_address_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Address"),
        syn::Type::Reference(reference) => is_address_type(&reference.elem),
        _ => false,
    }
}

fn is_owner_transfer_function(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let ownership_term = lower.contains("owner") || lower.contains("admin");
    let transfer_term = [
        "change", "grant", "replace", "rotate", "set", "transfer", "update",
    ]
    .iter()
    .any(|term| lower.contains(term));

    ownership_term && transfer_term
}

fn is_two_step_function_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "accept", "claim", "nominate", "pending", "propose", "request",
    ]
    .iter()
    .any(|term| lower.contains(term))
}

fn is_owner_param(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains("owner") || lower.contains("admin")
}

fn body_uses_pending_owner(body: &str) -> bool {
    let compact = compact_lower(body);
    compact.contains("pendingowner")
        || compact.contains("pendingadmin")
        || compact.contains("proposedowner")
        || compact.contains("proposedadmin")
}

fn body_writes_owner_slot(body: &str) -> bool {
    let compact = compact_lower(body);
    compact.contains(".set(")
        && (compact.contains("owner") || compact.contains("admin"))
        && !body_uses_pending_owner(body)
}

fn compact_lower(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '_')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
    })
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }

        match &attr.meta {
            syn::Meta::List(list) => list
                .tokens
                .to_string()
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .any(|part| part == "test"),
            _ => false,
        }
    })
}

fn suppressions(source: &str) -> Vec<usize> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.contains("sanctifier:ignore[SANCT_OWNERSHIP_TRANSFER]")
                .then_some(index + 1)
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions
        .iter()
        .any(|suppressed_line| *suppressed_line == line || *suppressed_line + 1 == line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_one_step_owner_or_admin_transfer() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn transfer_ownership(env: Env, new_owner: Address) {
                    env.storage().instance().set(&"owner", &new_owner);
                }

                pub fn set_admin(env: Env, new_admin: Address) {
                    env.storage().instance().set(&"admin", &new_admin);
                }
            }
        "#;

        let findings = OwnershipTransferRule::new().check(source);

        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|finding| finding.rule_name == OWNERSHIP_TRANSFER));
    }

    #[test]
    fn allows_two_step_and_suppressed_transfer() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn propose_owner(env: Env, new_owner: Address) {
                    env.storage().instance().set(&"pending_owner", &new_owner);
                }

                pub fn accept_ownership(env: Env) {
                    let pending_owner: Address = env.storage().instance().get(&"pending_owner").unwrap();
                    env.storage().instance().set(&"owner", &pending_owner);
                }

                // sanctifier:ignore[SANCT_OWNERSHIP_TRANSFER]
                pub fn set_admin(env: Env, new_admin: Address) {
                    env.storage().instance().set(&"admin", &new_admin);
                }
            }
        "#;

        let findings = OwnershipTransferRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
