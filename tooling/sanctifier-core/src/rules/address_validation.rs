use crate::finding_codes::ADDRESS_VALIDATION;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects sensitive contract entrypoints that accept Address inputs without an
/// explicit validity or zero-address guard.
pub struct AddressValidationRule;

impl AddressValidationRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AddressValidationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for AddressValidationRule {
    fn name(&self) -> &str {
        "address_validation"
    }

    fn description(&self) -> &str {
        "Detects sensitive Address inputs missing validity or zero-address validation"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractAddressVisitor {
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

struct ContractAddressVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ContractAddressVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ContractAddressVisitor {
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

                self.violations.extend(
                    check_function(function)
                        .into_iter()
                        .filter(|finding| !is_suppressed(&self.suppressions, finding.line))
                        .map(AddressIssue::into_violation),
                );
            }
        }
    }
}

fn check_function(function: &syn::ImplItemFn) -> Vec<AddressIssue> {
    let fn_name = function.sig.ident.to_string();
    let sensitive_fn = is_sensitive_function(&fn_name);
    let body = quote::quote!(#function.block).to_string();

    address_params(&function.sig)
        .into_iter()
        .filter(|param| sensitive_fn || is_sensitive_address_name(&param.name))
        .filter(|param| !body_validates_address(&body, &param.name))
        .map(|param| AddressIssue {
            fn_name: fn_name.clone(),
            param_name: param.name,
            line: param.line,
        })
        .collect()
}

#[derive(Debug, Clone)]
struct AddressParam {
    name: String,
    line: usize,
}

struct AddressIssue {
    fn_name: String,
    param_name: String,
    line: usize,
}

impl AddressIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            ADDRESS_VALIDATION,
            Severity::Warning,
            format!(
                "{ADDRESS_VALIDATION}: Address parameter `{}` in `{}` is not explicitly validated",
                self.param_name, self.fn_name
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(format!(
            "Validate `{}` with an explicit zero-address/invalid-address guard before storing it or moving value",
            self.param_name
        ))
    }
}

fn address_params(sig: &syn::Signature) -> Vec<AddressParam> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) if is_address_type(&pat_type.ty) => {
                pat_ident(&pat_type.pat).map(|name| AddressParam {
                    name,
                    line: pat_type.span().start().line,
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

fn is_sensitive_function(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "admin",
        "asset",
        "beneficiary",
        "claim",
        "config",
        "deposit",
        "grant",
        "init",
        "mint",
        "owner",
        "pause",
        "recipient",
        "revoke",
        "set",
        "transfer",
        "update",
        "upgrade",
        "withdraw",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

fn is_sensitive_address_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        "admin",
        "asset",
        "beneficiary",
        "operator",
        "owner",
        "recipient",
        "receiver",
        "spender",
        "token",
        "to",
        "treasury",
        "user",
    ]
    .iter()
    .any(|keyword| lower == *keyword || lower.contains(keyword))
}

fn body_validates_address(body: &str, arg_name: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    let arg = arg_name.to_ascii_lowercase();
    if !lower.contains(&arg) {
        return false;
    }

    let validation_keywords = [
        "assert_not_zero",
        "assert_valid_address",
        "ensure_not_zero",
        "ensure_valid_address",
        "invalid_address",
        "is_valid_address",
        "reject_zero_address",
        "require_valid_address",
        "validate_address",
        "zero_address",
    ];

    if validation_keywords
        .iter()
        .any(|keyword| lower.contains(keyword) && nearby_argument(&lower, keyword, &arg))
    {
        return true;
    }

    let compact: String = lower.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.contains(&format!("{arg}.is_zero("))
        || compact.contains(&format!("!{arg}.is_valid("))
        || compact.contains(&format!("{arg}==address::zero"))
        || compact.contains(&format!("{arg}!=address::zero"))
}

fn nearby_argument(source: &str, keyword: &str, arg: &str) -> bool {
    source.match_indices(keyword).any(|(index, _)| {
        let start = index.saturating_sub(96);
        let end = (index + keyword.len() + 96).min(source.len());
        source[start..end].contains(arg)
    })
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
            line.contains("sanctifier:ignore[SANCT_ADDRESS_VALIDATION]")
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
    fn detects_unvalidated_sensitive_addresses() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn set_admin(env: Env, admin: Address) {
                    env.storage().instance().set(&"admin", &admin);
                }

                pub fn set_payout(env: Env, recipient: Address, token: Address) {
                    env.storage().instance().set(&"recipient", &recipient);
                    env.storage().instance().set(&"token", &token);
                }
            }
        "#;

        let findings = AddressValidationRule::new().check(source);

        assert_eq!(findings.len(), 3, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|finding| finding.rule_name == ADDRESS_VALIDATION));
    }

    #[test]
    fn allows_explicit_validation_and_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn set_admin(env: Env, admin: Address) {
                    validate_address(&admin);
                    env.storage().instance().set(&"admin", &admin);
                }

                pub fn set_recipient(env: Env, recipient: Address) {
                    if recipient.is_zero() {
                        panic!("zero recipient");
                    }
                    env.storage().instance().set(&"recipient", &recipient);
                }

                // sanctifier:ignore[SANCT_ADDRESS_VALIDATION]
                pub fn set_token(env: Env, token: Address) {
                    env.storage().instance().set(&"token", &token);
                }
            }
        "#;

        let findings = AddressValidationRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
