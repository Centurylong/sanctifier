use crate::finding_codes::SENSITIVE_GETTER;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{Attribute, Visibility};

pub struct SensitiveGetterRule;

impl SensitiveGetterRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SensitiveGetterRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for SensitiveGetterRule {
    fn name(&self) -> &str {
        "sensitive_getter"
    }

    fn description(&self) -> &str {
        "Detects public getters that expose secret, credential, or signing-key-shaped storage fields"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };
        let mut visitor = ContractVisitor {
            violations: Vec::new(),
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
            let syn::ImplItem::Fn(function) = item else {
                continue;
            };
            if !matches!(function.vis, Visibility::Public(_))
                || matches!(function.sig.output, syn::ReturnType::Default)
            {
                continue;
            }

            let fn_name = function.sig.ident.to_string();
            if !is_getter_like(&fn_name) {
                continue;
            }

            let function_name_is_sensitive = is_sensitive_name(&fn_name);
            let reads = returned_storage_reads(function);

            if let Some(read) = reads
                .into_iter()
                .find(|read| function_name_is_sensitive || is_sensitive_name(&read.label))
            {
                self.violations.push(
                    RuleViolation::new(
                        SENSITIVE_GETTER,
                        Severity::Info,
                        format!(
                            "{SENSITIVE_GETTER}: public getter `{fn_name}` returns `{}` whose name indicates sensitive internal data",
                            read.label
                        ),
                        format!("{fn_name}:{}", read.line),
                    )
                    .with_suggestion(
                        "Do not return secrets, credentials, signing material, or admin-only keys from a public entrypoint. Keep them private/off-chain or expose only non-sensitive derived metadata.".to_string(),
                    ),
                );
            }
        }
    }
}

fn returned_storage_reads(function: &syn::ImplItemFn) -> Vec<StorageRead> {
    let mut explicit_returns = ReturnReadVisitor { reads: Vec::new() };
    explicit_returns.visit_block(&function.block);
    let mut reads = explicit_returns.reads;

    if let Some(syn::Stmt::Expr(expr, None)) = function.block.stmts.last() {
        if !matches!(expr, syn::Expr::Return(_)) {
            let mut tail_reads = StorageReadVisitor { reads: Vec::new() };
            tail_reads.visit_expr(expr);
            reads.extend(tail_reads.reads);
        }
    }

    reads
}

struct ReturnReadVisitor {
    reads: Vec<StorageRead>,
}

impl<'ast> Visit<'ast> for ReturnReadVisitor {
    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        if let Some(expr) = &node.expr {
            let mut returned = StorageReadVisitor { reads: Vec::new() };
            returned.visit_expr(expr);
            self.reads.extend(returned.reads);
        }
    }
}

struct StorageReadVisitor {
    reads: Vec<StorageRead>,
}

struct StorageRead {
    label: String,
    line: usize,
}

impl<'ast> Visit<'ast> for StorageReadVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "get" && is_storage_receiver(&node.receiver) {
            let label = node
                .args
                .first()
                .map(|arg| quote::quote!(#arg).to_string())
                .unwrap_or_else(|| "storage value".to_string());
            self.reads.push(StorageRead {
                label: label
                    .strip_prefix("& ")
                    .unwrap_or(label.as_str())
                    .to_string(),
                line: node.method.span().start().line,
            });
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn is_storage_receiver(receiver: &syn::Expr) -> bool {
    let rendered = quote::quote!(#receiver).to_string();
    rendered.contains("storage")
        && (rendered.contains("persistent")
            || rendered.contains("temporary")
            || rendered.contains("instance"))
}

fn is_getter_like(name: &str) -> bool {
    const PREFIXES: &[&str] = &["get_", "view_", "read_", "fetch_", "query_", "peek_"];
    PREFIXES.iter().any(|prefix| name.starts_with(prefix)) || is_sensitive_name(name)
}

fn is_sensitive_name(value: &str) -> bool {
    const TERMS: &[&str] = &[
        "secret",
        "credential",
        "password",
        "mnemonic",
        "privatekey",
        "signingkey",
        "adminkey",
        "masterkey",
        "recoverykey",
        "apikey",
        "accesstoken",
        "refreshtoken",
        "authtoken",
        "encryptionkey",
        "decryptionkey",
        "secretkey",
        "seedphrase",
        "walletseed",
    ];
    let normalized: String = value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    TERMS.iter().any(|term| normalized.contains(term))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_sensitive_key_getter() {
        let source = r#"
            use soroban_sdk::{contractimpl, BytesN, Env};
            #[contractimpl]
            impl Contract {
                pub fn get_signing_key(env: Env) -> BytesN<32> {
                    env.storage().instance().get(&DataKey::SigningKey).unwrap()
                }
            }
        "#;
        let findings = SensitiveGetterRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, SENSITIVE_GETTER);
        assert_eq!(findings[0].severity, Severity::Info);
    }

    #[test]
    fn ignores_ordinary_public_metadata() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};
            #[contractimpl]
            impl Contract {
                pub fn get_owner(env: Env) -> Address {
                    env.storage().instance().get(&DataKey::Owner).unwrap()
                }
            }
        "#;
        assert!(SensitiveGetterRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_sensitive_read_that_is_not_returned() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};
            #[contractimpl]
            impl Contract {
                pub fn get_owner(env: Env) -> Address {
                    let _signing_key =
                        env.storage().instance().get(&DataKey::SigningKey);
                    env.storage().instance().get(&DataKey::Owner).unwrap()
                }
            }
        "#;
        assert!(SensitiveGetterRule::new().check(source).is_empty());
    }
}
