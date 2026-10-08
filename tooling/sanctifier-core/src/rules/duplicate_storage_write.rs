use crate::finding_codes::DUPLICATE_STORAGE_WRITE;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::Attribute;

/// Detects redundant Soroban storage writes in one straight-line sequence.
///
/// The rule is intentionally conservative. It reports only when two storage
/// `set` calls target the same storage kind/key with the same stable value
/// expression and no invalidating statement occurs between them. A write to a
/// different-looking key in the same storage namespace may alias the earlier
/// key, so it also invalidates the earlier candidate. Writing a new value,
/// mutating a local first, or crossing a control-flow boundary is treated as
/// an intentional update and is not reported.
pub struct DuplicateStorageWriteRule;

impl DuplicateStorageWriteRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DuplicateStorageWriteRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DuplicateStorageWriteRule {
    fn name(&self) -> &str {
        "duplicate_storage_write"
    }

    fn description(&self) -> &str {
        "Detects redundant repeated writes of the same value to the same Soroban storage key"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };

        let mut visitor = DuplicateWriteVisitor {
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

struct DuplicateWriteVisitor {
    violations: Vec<RuleViolation>,
    test_depth: usize,
}

impl DuplicateWriteVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }

    fn analyze_function(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let env_params = env_parameter_names(sig);
        if env_params.is_empty() {
            return;
        }

        let function_name = sig.ident.to_string();
        let mut blocks = BlockVisitor {
            function_name: &function_name,
            env_params: &env_params,
            violations: &mut self.violations,
        };
        blocks.visit_block(block);
    }
}

impl<'ast> Visit<'ast> for DuplicateWriteVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_test = has_cfg_test(&node.attrs);
        if was_test {
            self.test_depth += 1;
        }
        visit::visit_item_mod(self, node);
        if was_test {
            self.test_depth -= 1;
        }
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if !self.in_test_module() {
            self.analyze_function(&node.sig, &node.block);
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if !self.in_test_module() {
            self.analyze_function(&node.sig, &node.block);
        }
    }
}

struct BlockVisitor<'a> {
    function_name: &'a str,
    env_params: &'a HashSet<String>,
    violations: &'a mut Vec<RuleViolation>,
}

impl BlockVisitor<'_> {
    /// A nested function or impl method owns its own parameters and finding
    /// label. Never reuse the enclosing function's Env names in its body.
    fn visit_scoped_function(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let env_params = env_parameter_names(sig);
        if env_params.is_empty() {
            return;
        }

        let function_name = sig.ident.to_string();
        let mut nested = BlockVisitor {
            function_name: &function_name,
            env_params: &env_params,
            violations: &mut *self.violations,
        };
        nested.visit_block(block);
    }
}

impl<'ast> Visit<'ast> for BlockVisitor<'_> {
    fn visit_block(&mut self, node: &'ast syn::Block) {
        analyze_direct_statements(
            self.function_name,
            node,
            self.env_params,
            self.violations,
        );
        visit::visit_block(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.visit_scoped_function(&node.sig, &node.block);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.visit_scoped_function(&node.sig, &node.block);
    }
}

struct StorageWrite {
    kind: &'static str,
    key: String,
    display_key: String,
    value: String,
    line: usize,
}

fn analyze_direct_statements(
    fn_name: &str,
    block: &syn::Block,
    env_params: &HashSet<String>,
    violations: &mut Vec<RuleViolation>,
) {
    let mut last_by_target: HashMap<(String, String), StorageWrite> = HashMap::new();

    for statement in &block.stmts {
        let Some(write) = direct_storage_set(statement, env_params) else {
            last_by_target.clear();
            continue;
        };

        let target = (write.kind.to_string(), write.key.clone());
        // Distinct key expressions can resolve to the same runtime Symbol.
        // A write to a different-looking key in this storage namespace might
        // overwrite the prior key, making a later restoration write necessary.
        // Keep independent namespaces (instance/persistent/temporary) separate.
        last_by_target.retain(|(kind, key), _| {
            kind.as_str() != write.kind || key == &write.key
        });
        if let Some(previous) = last_by_target.get(&target) {
            if previous.value == write.value {
                violations.push(
                    RuleViolation::new(
                        DUPLICATE_STORAGE_WRITE,
                        Severity::Warning,
                        format!(
                            "{DUPLICATE_STORAGE_WRITE}: `{fn_name}` writes the same value to storage key `{}` more than once without an intervening state/value change",
                            write.display_key
                        ),
                        format!("{fn_name}:{}", write.line),
                    )
                    .with_suggestion(
                        "Keep one write when the key/value pair is unchanged. If the second write is an intentional update, compute or mutate the new value before writing it.".to_string(),
                    ),
                );
            }
        }

        last_by_target.insert(target, write);
    }
}

fn direct_storage_set(
    statement: &syn::Stmt,
    env_params: &HashSet<String>,
) -> Option<StorageWrite> {
    let syn::Stmt::Expr(syn::Expr::MethodCall(call), _) = statement else {
        return None;
    };
    if call.method != "set" || call.args.len() != 2 {
        return None;
    }

    let kind = storage_kind(&call.receiver, env_params)?;
    let mut args = call.args.iter();
    let key_expr = args.next()?;
    let value_expr = args.next()?;

    if !is_stable_expr(key_expr) || !is_stable_expr(value_expr) {
        return None;
    }

    Some(StorageWrite {
        kind,
        key: canonical_expr(key_expr),
        display_key: display_expr(key_expr),
        value: canonical_expr(value_expr),
        line: call.method.span().start().line,
    })
}

fn env_parameter_names(sig: &syn::Signature) -> HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(arg) = arg else {
                return None;
            };
            if !is_env_type(&arg.ty) {
                return None;
            }
            let syn::Pat::Ident(ident) = arg.pat.as_ref() else {
                return None;
            };
            Some(ident.ident.to_string())
        })
        .collect()
}

fn is_env_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Env"),
        syn::Type::Reference(reference) => is_env_type(&reference.elem),
        syn::Type::Paren(paren) => is_env_type(&paren.elem),
        syn::Type::Group(group) => is_env_type(&group.elem),
        _ => false,
    }
}

fn storage_kind(
    expr: &syn::Expr,
    env_params: &HashSet<String>,
) -> Option<&'static str> {
    let syn::Expr::MethodCall(call) = expr else {
        return None;
    };

    let kind = if call.method == "persistent" {
        Some("persistent")
    } else if call.method == "temporary" {
        Some("temporary")
    } else if call.method == "instance" {
        Some("instance")
    } else {
        None
    };

    if let Some(kind) = kind {
        return storage_env_root(&call.receiver)
            .filter(|name| env_params.contains(name))
            .map(|_| kind);
    }

    storage_kind(&call.receiver, env_params)
}

fn storage_env_root(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(call) if call.method == "storage" => {
            env_receiver_name(&call.receiver)
        }
        syn::Expr::MethodCall(call) => storage_env_root(&call.receiver),
        syn::Expr::Reference(reference) => storage_env_root(&reference.expr),
        syn::Expr::Paren(paren) => storage_env_root(&paren.expr),
        syn::Expr::Group(group) => storage_env_root(&group.expr),
        _ => None,
    }
}

fn env_receiver_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.qself.is_none() => {
            path.path.get_ident().map(ToString::to_string)
        }
        syn::Expr::Reference(reference) => env_receiver_name(&reference.expr),
        syn::Expr::Paren(paren) => env_receiver_name(&paren.expr),
        syn::Expr::Group(group) => env_receiver_name(&group.expr),
        _ => None,
    }
}

fn is_stable_expr(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Reference(reference) => is_stable_expr(&reference.expr),
        syn::Expr::Path(_) | syn::Expr::Lit(_) => true,
        syn::Expr::Paren(paren) => is_stable_expr(&paren.expr),
        syn::Expr::Group(group) => is_stable_expr(&group.expr),
        syn::Expr::Field(field) => is_stable_expr(&field.base),
        syn::Expr::Index(index) => is_stable_expr(&index.expr) && is_stable_expr(&index.index),
        syn::Expr::Unary(unary) => is_stable_expr(&unary.expr),
        syn::Expr::Binary(binary) => {
            is_stable_expr(&binary.left) && is_stable_expr(&binary.right)
        }
        syn::Expr::Cast(cast) => is_stable_expr(&cast.expr),
        syn::Expr::Tuple(tuple) => tuple.elems.iter().all(is_stable_expr),
        syn::Expr::Array(array) => array.elems.iter().all(is_stable_expr),
        syn::Expr::Struct(struct_expr) => {
            struct_expr.fields.iter().all(|field| is_stable_expr(&field.expr))
                && struct_expr.rest.as_deref().map_or(true, is_stable_expr)
        }
        _ => false,
    }
}

fn canonical_expr(expr: &syn::Expr) -> String {
    quote::quote!(#expr)
        .to_string()
        .split_whitespace()
        .collect::<String>()
}

fn display_expr(expr: &syn::Expr) -> String {
    let rendered = quote::quote!(#expr).to_string();
    rendered
        .strip_prefix("& ")
        .unwrap_or(rendered.as_str())
        .to_string()
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
    fn flags_redundant_write_to_same_key_and_value() {
        let source = r#"
            impl Contract {
                pub fn save(env: Env, key: Symbol, value: i128) {
                    env.storage().persistent().set(&key, &value);
                    env.storage().persistent().set(&key, &value);
                }
            }
        "#;

        let findings = DuplicateStorageWriteRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, DUPLICATE_STORAGE_WRITE);
    }

    #[test]
    fn ignores_intentional_update_to_new_value() {
        let source = r#"
            impl Contract {
                pub fn save(env: Env, key: Symbol, first: i128, second: i128) {
                    env.storage().persistent().set(&key, &first);
                    env.storage().persistent().set(&key, &second);
                }
            }
        "#;

        assert!(DuplicateStorageWriteRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_write_after_value_mutation() {
        let source = r#"
            impl Contract {
                pub fn save(env: Env, key: Symbol, mut value: i128) {
                    env.storage().persistent().set(&key, &value);
                    value += 1;
                    env.storage().persistent().set(&key, &value);
                }
            }
        "#;

        assert!(DuplicateStorageWriteRule::new().check(source).is_empty());
    }
    #[test]
    fn ignores_non_soroban_persistent_receiver() {
        let source = r#"
            impl CacheUser {
                pub fn save(cache: Cache, key: Symbol, value: i128) {
                    cache.persistent().set(&key, &value);
                    cache.persistent().set(&key, &value);
                }
            }
        "#;

        assert!(
            DuplicateStorageWriteRule::new().check(source).is_empty(),
            "custom APIs named persistent().set() must not be treated as Soroban storage"
        );
    }

    #[test]
    fn ignores_custom_storage_hop_with_soroban_shaped_method_names() {
        let source = r#"
            impl CacheUser {
                pub fn save(cache: Cache, key: Symbol, value: i128) {
                    cache.storage().persistent().set(&key, &value);
                    cache.storage().persistent().set(&key, &value);
                }
            }
        "#;

        assert!(
            DuplicateStorageWriteRule::new().check(source).is_empty(),
            "a custom storage().persistent().set() chain must not be treated as Soroban storage"
        );
    }

    #[test]
    fn ignores_restore_after_potentially_aliasing_key_write() {
        let source = r#"
            impl Contract {
                pub fn save(env: Env, left: Symbol, right: Symbol, first: i128, other: i128) {
                    env.storage().persistent().set(&left, &first);
                    env.storage().persistent().set(&right, &other);
                    // right may equal left, so restoring first is essential.
                    env.storage().persistent().set(&left, &first);
                }
            }
        "#;

        assert!(
            DuplicateStorageWriteRule::new().check(source).is_empty(),
            "different key expressions may alias; do not remove a needed restoration write"
        );
    }

    #[test]
    fn nested_functions_use_their_own_env_types_and_names() {
        let source = r#"
            fn outer(env: Env, key: Symbol, value: i128) {
                fn cache_helper(env: Cache, key: Symbol, value: i128) {
                    env.storage().persistent().set(&key, &value);
                    env.storage().persistent().set(&key, &value);
                }

                fn nested(real_env: Env, key: Symbol, value: i128) {
                    real_env.storage().persistent().set(&key, &value);
                    real_env.storage().persistent().set(&key, &value);
                }
            }
        "#;

        let findings = DuplicateStorageWriteRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(
            findings[0].location.starts_with("nested:"),
            "the nested function must own its diagnostic label: {findings:#?}"
        );
    }

    #[test]
    fn detects_renamed_env_parameter() {
        let source = r#"
            impl Contract {
                pub fn save(context: soroban_sdk::Env, key: Symbol, value: i128) {
                    context.storage().persistent().set(&key, &value);
                    context.storage().persistent().set(&key, &value);
                }
            }
        "#;

        let findings = DuplicateStorageWriteRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
    }

}
