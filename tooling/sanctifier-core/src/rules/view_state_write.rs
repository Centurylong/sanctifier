use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_VIEW_STATE_WRITE";

/// Detects getter-like public functions that write to contract storage.
pub struct ViewStateWriteRule;

impl ViewStateWriteRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        fn_name: &str,
        visibility: &syn::Visibility,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        if !matches!(visibility, syn::Visibility::Public(_)) || !is_getter_like_name(fn_name) {
            return Vec::new();
        }

        let mut visitor = StorageWriteVisitor::default();
        visitor.visit_block(block);

        visitor
            .writes
            .into_iter()
            .map(|write| {
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    format!(
                        "{FINDING_CODE}: getter-like function `{fn_name}` writes {kind} storage with `{method}`",
                        kind = write.kind,
                        method = write.method
                    ),
                    format!("{}:{}", fn_name, write.line),
                )
                .with_suggestion(
                    "Keep getter/read functions side-effect free, or rename this entrypoint to make the storage mutation explicit"
                        .to_string(),
                )
            })
            .collect()
    }
}

impl Default for ViewStateWriteRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ViewStateWriteRule {
    fn name(&self) -> &str {
        "view_state_write"
    }

    fn description(&self) -> &str {
        "Detects getter-like public functions that mutate contract storage"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = ViewStateWriteRuleVisitor {
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

struct ViewStateWriteRuleVisitor<'rule> {
    rule: &'rule ViewStateWriteRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for ViewStateWriteRuleVisitor<'_> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.block,
        ));
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.block,
        ));
        syn::visit::visit_item_fn(self, node);
    }
}

#[derive(Default)]
struct StorageWriteVisitor {
    writes: Vec<StorageWrite>,
}

#[derive(Debug, Clone)]
struct StorageWrite {
    kind: &'static str,
    method: String,
    line: usize,
}

impl<'ast> Visit<'ast> for StorageWriteVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if is_storage_write_method(&method) {
            if let Some(kind) = storage_kind(&node.receiver) {
                self.writes.push(StorageWrite {
                    kind,
                    method,
                    line: node.method.span().start().line,
                });
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

fn is_storage_write_method(method: &str) -> bool {
    matches!(method, "set" | "update" | "remove" | "extend_ttl")
}

fn storage_kind(expr: &syn::Expr) -> Option<&'static str> {
    match expr {
        syn::Expr::MethodCall(method_call) if method_call.method == "persistent" => {
            Some("persistent")
        }
        syn::Expr::MethodCall(method_call) if method_call.method == "temporary" => {
            Some("temporary")
        }
        syn::Expr::MethodCall(method_call) if method_call.method == "instance" => Some("instance"),
        syn::Expr::MethodCall(method_call) => storage_kind(&method_call.receiver),
        _ => None,
    }
}

fn is_getter_like_name(fn_name: &str) -> bool {
    let name = fn_name.to_ascii_lowercase();
    if has_mutating_prefix(&name) {
        return false;
    }

    has_getter_prefix(&name) || has_getter_token(&name)
}

fn has_mutating_prefix(name: &str) -> bool {
    [
        "set",
        "put",
        "save",
        "store",
        "write",
        "record",
        "create",
        "add",
        "remove",
        "delete",
        "update",
        "increment",
        "decrement",
        "bump",
        "extend",
        "transfer",
        "mint",
        "burn",
        "approve",
        "withdraw",
        "deposit",
        "claim",
        "initialize",
        "init",
        "migrate",
        "upgrade",
        "admin",
    ]
    .iter()
    .any(|prefix| name == *prefix || name.starts_with(&format!("{prefix}_")))
}

fn has_getter_prefix(name: &str) -> bool {
    [
        "get_", "read_", "view_", "query_", "fetch_", "load_", "preview_", "quote_", "current_",
        "is_", "has_", "can_",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}

fn has_getter_token(name: &str) -> bool {
    [
        "getter",
        "balance",
        "allowance",
        "supply",
        "total_supply",
        "symbol",
        "name",
        "decimals",
        "metadata",
        "status",
    ]
    .iter()
    .any(|token| name == *token || name.contains(&format!("_{token}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_getter_that_sets_storage() {
        let source = r#"
            impl Contract {
                pub fn get_balance(env: Env, user: Address) -> i128 {
                    env.storage().persistent().set(&user, &0);
                    0
                }
            }
        "#;

        let findings = ViewStateWriteRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].location.contains("get_balance"));
    }

    #[test]
    fn flags_preview_that_removes_temporary_storage() {
        let source = r#"
            impl Contract {
                pub fn preview_rewards(env: Env, user: Address) -> i128 {
                    env.storage().temporary().remove(&user);
                    0
                }
            }
        "#;

        let findings = ViewStateWriteRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("temporary"));
    }

    #[test]
    fn allows_intended_mutation_names() {
        let source = r#"
            impl Contract {
                pub fn set_balance(env: Env, user: Address, value: i128) {
                    env.storage().persistent().set(&user, &value);
                }
            }
        "#;

        let findings = ViewStateWriteRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn allows_pure_getter_reads() {
        let source = r#"
            impl Contract {
                pub fn get_balance(env: Env, user: Address) -> i128 {
                    env.storage().persistent().get(&user).unwrap_or(0)
                }
            }
        "#;

        let findings = ViewStateWriteRule::new().check(source);

        assert!(findings.is_empty());
    }
}
