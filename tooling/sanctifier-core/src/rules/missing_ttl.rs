use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_TTL_MISSING";

/// Detects persistent/instance storage accesses that do not extend the same key's TTL.
pub struct MissingTtlRule;

impl MissingTtlRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(&self, fn_name: &str, block: &syn::Block) -> Vec<RuleViolation> {
        let mut visitor = StorageTtlVisitor::default();
        visitor.visit_block(block);

        visitor
            .accesses
            .into_iter()
            .filter(|access| !visitor.bumps.iter().any(|bump| bump.covers(access)))
            .map(|access| {
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    access.message(),
                    format!("{}:{}", fn_name, access.line),
                )
                .with_suggestion(access.suggestion())
            })
            .collect()
    }
}

impl Default for MissingTtlRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for MissingTtlRule {
    fn name(&self) -> &str {
        "missing_ttl"
    }

    fn description(&self) -> &str {
        "Detects persistent/instance storage access without a matching TTL extension"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = MissingTtlRuleVisitor {
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

struct MissingTtlRuleVisitor<'rule> {
    rule: &'rule MissingTtlRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for MissingTtlRuleVisitor<'_> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(
            self.rule
                .check_function(&node.sig.ident.to_string(), &node.block),
        );
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(
            self.rule
                .check_function(&node.sig.ident.to_string(), &node.block),
        );
        syn::visit::visit_item_fn(self, node);
    }
}

#[derive(Default)]
struct StorageTtlVisitor {
    accesses: Vec<StorageAccess>,
    bumps: Vec<StorageBump>,
}

impl<'ast> Visit<'ast> for StorageTtlVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(kind) = storage_kind(&node.receiver) {
            if let Some(key_expr) = node.args.first() {
                let method = node.method.to_string();
                if is_ttl_bump_method(&method) {
                    self.bumps.push(StorageBump::new(kind, key_expr));
                } else if is_durable_access_method(&method) {
                    self.accesses.push(StorageAccess::new(
                        kind,
                        key_expr,
                        method,
                        node.span().start().line,
                    ));
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

#[derive(Debug, Clone)]
struct StorageAccess {
    kind: StorageKind,
    key: Option<String>,
    display_key: String,
    method: String,
    line: usize,
}

impl StorageAccess {
    fn new(kind: StorageKind, key_expr: &syn::Expr, method: String, line: usize) -> Self {
        Self {
            kind,
            key: durable_key(kind, key_expr),
            display_key: display_key(key_expr),
            method,
            line,
        }
    }

    fn message(&self) -> String {
        match self.kind {
            StorageKind::Persistent => format!(
                "SANCT_TTL_MISSING: persistent storage `{}` for key `{}` does not extend TTL",
                self.method, self.display_key
            ),
            StorageKind::Instance => format!(
                "SANCT_TTL_MISSING: instance storage `{}` for key `{}` does not extend TTL",
                self.method, self.display_key
            ),
        }
    }

    fn suggestion(&self) -> String {
        match self.kind {
            StorageKind::Persistent => format!(
                "Call `env.storage().persistent().extend_ttl(&{}, LOW, HIGH)` after accessing this durable entry",
                self.display_key
            ),
            StorageKind::Instance => {
                "Call `env.storage().instance().extend_ttl(LOW, HIGH)` after accessing instance storage".to_string()
            }
        }
    }
}

#[derive(Debug, Clone)]
struct StorageBump {
    kind: StorageKind,
    key: Option<String>,
}

impl StorageBump {
    fn new(kind: StorageKind, key_expr: &syn::Expr) -> Self {
        Self {
            kind,
            key: durable_key(kind, key_expr),
        }
    }

    fn covers(&self, access: &StorageAccess) -> bool {
        if self.kind != access.kind {
            return false;
        }

        match self.kind {
            StorageKind::Persistent => self.key == access.key,
            StorageKind::Instance => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StorageKind {
    Persistent,
    Instance,
}

fn is_durable_access_method(method: &str) -> bool {
    matches!(method, "get" | "set" | "update" | "remove" | "has")
}

fn is_ttl_bump_method(method: &str) -> bool {
    method == "extend_ttl" || method.contains("bump")
}

fn storage_kind(expr: &syn::Expr) -> Option<StorageKind> {
    match expr {
        syn::Expr::MethodCall(method_call) if method_call.method == "persistent" => {
            Some(StorageKind::Persistent)
        }
        syn::Expr::MethodCall(method_call) if method_call.method == "instance" => {
            Some(StorageKind::Instance)
        }
        syn::Expr::MethodCall(method_call) => storage_kind(&method_call.receiver),
        _ => None,
    }
}

fn durable_key(kind: StorageKind, expr: &syn::Expr) -> Option<String> {
    match kind {
        StorageKind::Persistent => Some(display_key(expr).split_whitespace().collect()),
        StorageKind::Instance => None,
    }
}

fn display_key(expr: &syn::Expr) -> String {
    let key_expr = match expr {
        syn::Expr::Reference(reference) => reference.expr.as_ref(),
        _ => expr,
    };
    quote::quote!(#key_expr).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_persistent_access_without_ttl_extension() {
        let source = r#"
            impl Contract {
                pub fn write(env: Env, key: Symbol, value: i128) {
                    env.storage().persistent().set(&key, &value);
                }
            }
        "#;

        let findings = MissingTtlRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, "SANCT_TTL_MISSING");
        assert!(findings[0].message.contains("SANCT_TTL_MISSING"));
    }

    #[test]
    fn accepts_matching_ttl_extension_for_same_key_and_kind() {
        let source = r#"
            impl Contract {
                pub fn write(env: Env, key: Symbol, value: i128) {
                    env.storage().persistent().set(&key, &value);
                    env.storage().persistent().extend_ttl(&key, 100, 1000);
                }
            }
        "#;

        let findings = MissingTtlRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn requires_extension_on_same_storage_kind() {
        let source = r#"
            impl Contract {
                pub fn write(env: Env, key: Symbol, value: i128) {
                    env.storage().persistent().set(&key, &value);
                    env.storage().instance().extend_ttl(&key, 100, 1000);
                }
            }
        "#;

        let findings = MissingTtlRule::new().check(source);

        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn accepts_instance_ttl_extension_without_key_match() {
        let source = r#"
            impl Contract {
                pub fn write(env: Env, key: Symbol, value: i128) {
                    env.storage().instance().set(&key, &value);
                    env.storage().instance().extend_ttl(100, 1000);
                }
            }
        "#;

        let findings = MissingTtlRule::new().check(source);

        assert!(findings.is_empty());
    }
}
