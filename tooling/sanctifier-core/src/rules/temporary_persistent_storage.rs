use crate::finding_codes::TEMPORARY_PERSISTENT_STORAGE;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects durable contract state written through temporary storage.
pub struct TemporaryPersistentStorageRule;

impl TemporaryPersistentStorageRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TemporaryPersistentStorageRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for TemporaryPersistentStorageRule {
    fn name(&self) -> &str {
        "temporary_persistent_storage"
    }

    fn description(&self) -> &str {
        "Detects durable balances/config stored in temporary storage"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractVisitor {
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

struct ContractVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ContractVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ContractVisitor {
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

                let mut function_visitor = FunctionVisitor {
                    fn_name: function.sig.ident.to_string(),
                    issues: Vec::new(),
                };
                function_visitor.visit_block(&function.block);

                self.violations.extend(
                    function_visitor
                        .issues
                        .into_iter()
                        .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                        .map(StorageIssue::into_violation),
                );
            }
        }
    }
}

struct FunctionVisitor {
    fn_name: String,
    issues: Vec<StorageIssue>,
}

impl<'ast> Visit<'ast> for FunctionVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "set" {
            let receiver = compact_lower(&quote::quote!(#node.receiver).to_string());
            if receiver.contains(".storage().temporary()") {
                let call = compact_lower(&quote::quote!(#node).to_string());
                if let Some(keyword) = durable_keyword(&call) {
                    self.issues.push(StorageIssue {
                        fn_name: self.fn_name.clone(),
                        keyword: keyword.to_string(),
                        line: node.method.span().start().line,
                    });
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

struct StorageIssue {
    fn_name: String,
    keyword: String,
    line: usize,
}

impl StorageIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            TEMPORARY_PERSISTENT_STORAGE,
            Severity::Warning,
            format!(
                "{TEMPORARY_PERSISTENT_STORAGE}: `{}` stores durable `{}` data in temporary storage",
                self.fn_name, self.keyword
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(
            "Use persistent or instance storage for durable balances/config, or rename/suppress if the value is intentionally ephemeral"
                .to_string(),
        )
    }
}

fn durable_keyword(value: &str) -> Option<&'static str> {
    if ephemeral_keyword(value).is_some() {
        return None;
    }

    [
        "admin",
        "allowance",
        "balance",
        "beneficiary",
        "config",
        "owner",
        "position",
        "reserve",
        "share",
        "supply",
        "treasury",
    ]
    .into_iter()
    .find(|keyword| value.contains(keyword))
}

fn ephemeral_keyword(value: &str) -> Option<&'static str> {
    [
        "cache",
        "ephemeral",
        "preview",
        "scratch",
        "session",
        "transient",
    ]
    .into_iter()
    .find(|keyword| value.contains(keyword))
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
            line.contains("sanctifier:ignore[SANCT_TEMPORARY_PERSISTENT_STORAGE]")
                .then_some(index + 1)
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions
        .iter()
        .any(|suppressed_line| *suppressed_line <= line && line <= *suppressed_line + 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_durable_state_in_temporary_storage() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn set_balance(env: Env, user: Address, balance: i128) {
                    env.storage().temporary().set(&("balance", user), &balance);
                }

                pub fn configure(env: Env, owner: Address) {
                    env.storage().temporary().set(&"owner", &owner);
                }
            }
        "#;

        let findings = TemporaryPersistentStorageRule::new().check(source);

        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|finding| finding.rule_name == TEMPORARY_PERSISTENT_STORAGE));
    }

    #[test]
    fn allows_ephemeral_temporary_storage_and_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn cache_quote(env: Env, user: Address, balance_preview: i128) {
                    env.storage().temporary().set(&("cache_balance_preview", user), &balance_preview);
                }

                // sanctifier:ignore[SANCT_TEMPORARY_PERSISTENT_STORAGE]
                pub fn set_balance_for_test(env: Env, user: Address, balance: i128) {
                    env.storage().temporary().set(&("balance", user), &balance);
                }
            }
        "#;

        let findings = TemporaryPersistentStorageRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
