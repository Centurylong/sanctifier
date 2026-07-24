use crate::finding_codes::STATE_WRITE_IN_VIEW;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects storage mutations performed inside getter/view-style functions.
///
/// Soroban callers, indexers, and other contracts treat getter-named functions
/// (`get_*`, `is_*`, `balance_of`, ...) as read-only. A getter that silently
/// writes to storage violates that expectation and can surprise off-chain
/// consumers that assume calling it has no side effects.
///
/// Intended mutations are recognized and are *not* flagged:
///   * TTL maintenance (`extend_ttl`, `bump`, ...) is idiomatic on the read path
///     and is never treated as a data mutation — only `set`/`update`/`remove`
///     count as writes.
///   * A line annotated with `// sanctifier:ignore[SANCT_STATE_WRITE_IN_VIEW]`
///     opts out explicitly, for the rare getter that must write by design.
pub struct StateWriteInViewRule;

impl StateWriteInViewRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StateWriteInViewRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for StateWriteInViewRule {
    fn name(&self) -> &str {
        "state_write_in_view"
    }

    fn description(&self) -> &str {
        "Detects storage writes inside getter/view-style functions that callers expect to be read-only"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ViewWriteVisitor {
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

struct ViewWriteVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ViewWriteVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ViewWriteVisitor {
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
        if self.in_test_module() {
            return;
        }

        for item in &node.items {
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) {
                    continue;
                }
                let fn_name = function.sig.ident.to_string();
                if !is_view_like(&fn_name) {
                    continue;
                }

                let mut writes = FunctionWriteVisitor { hits: Vec::new() };
                writes.visit_block(&function.block);

                if let Some(hit) = writes
                    .hits
                    .into_iter()
                    .find(|hit| !is_suppressed(&self.suppressions, hit.line))
                {
                    self.violations.push(
                        RuleViolation::new(
                            STATE_WRITE_IN_VIEW,
                            Severity::Warning,
                            format!(
                                "{STATE_WRITE_IN_VIEW}: view/getter `{fn_name}` performs a storage `{}` write; callers and indexers expect getters to be read-only",
                                hit.method
                            ),
                            format!("{fn_name}:{}", hit.line),
                        )
                        .with_suggestion(
                            "Move the write into a dedicated state-changing function, rename the function, or annotate the line with `// sanctifier:ignore[SANCT_STATE_WRITE_IN_VIEW]` if it is intentional. TTL bumps such as extend_ttl() are never flagged.".to_string(),
                        ),
                    );
                }
            }
        }
    }
}

/// Collects storage-mutating method calls (`set`/`update`/`remove` on a storage
/// handle) within a single function body. TTL maintenance calls such as
/// `extend_ttl` are intentionally not matched — they are expected on reads.
struct FunctionWriteVisitor {
    hits: Vec<WriteHit>,
}

struct WriteHit {
    method: String,
    line: usize,
}

impl<'ast> Visit<'ast> for FunctionWriteVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if matches!(method.as_str(), "set" | "update" | "remove")
            && is_storage_receiver(&node.receiver)
        {
            self.hits.push(WriteHit {
                method,
                line: node.method.span().start().line,
            });
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Returns true for function names that callers conventionally treat as
/// read-only accessors.
fn is_view_like(name: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "get_", "view_", "read_", "is_", "has_", "query_", "fetch_", "peek_",
    ];
    // Canonical read-only accessors from the token / SEP interfaces.
    const EXACT: &[&str] = &[
        "balance",
        "allowance",
        "total_supply",
        "decimals",
        "symbol",
        "name",
        "owner",
        "admin",
        "version",
        "metadata",
        "count",
    ];

    let n = name.to_lowercase();
    PREFIXES.iter().any(|prefix| n.starts_with(prefix))
        || n.ends_with("_of")
        || EXACT.contains(&n.as_str())
}

/// True when the receiver chain of a method call touches a Soroban storage
/// handle (`storage()`, `persistent()`, `temporary()`, `instance()`).
fn is_storage_receiver(receiver: &syn::Expr) -> bool {
    let rendered = quote::quote!(#receiver).to_string();
    rendered.contains("storage")
        || rendered.contains("persistent")
        || rendered.contains("temporary")
        || rendered.contains("instance")
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
            line.contains("sanctifier:ignore[SANCT_STATE_WRITE_IN_VIEW]")
                .then_some(index + 1)
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions
        .iter()
        .any(|suppressed| *suppressed == line || *suppressed + 1 == line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_setter_in_getter() {
        let src = r#"
            impl C {
                pub fn get_balance(env: Env, who: Address) -> i128 {
                    env.storage().persistent().set(&who, &0i128);
                    0
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_name, STATE_WRITE_IN_VIEW);
        assert!(v[0].message.contains("get_balance"));
        assert!(v[0].message.contains("`set`"));
    }

    #[test]
    fn ignores_ttl_bump_in_getter() {
        let src = r#"
            impl C {
                pub fn get_config(env: Env) -> u32 {
                    let c: u32 = env.storage().instance().get(&KEY).unwrap();
                    env.storage().instance().extend_ttl(100, 100);
                    c
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert!(v.is_empty(), "TTL bumps must not be flagged: {v:#?}");
    }

    #[test]
    fn ignores_write_in_non_getter() {
        let src = r#"
            impl C {
                pub fn set_admin(env: Env, admin: Address) {
                    env.storage().instance().set(&KEY, &admin);
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert!(
            v.is_empty(),
            "non-getter functions are out of scope: {v:#?}"
        );
    }

    #[test]
    fn respects_inline_suppression() {
        let src = r#"
            impl C {
                pub fn get_or_init(env: Env) -> u32 {
                    // sanctifier:ignore[SANCT_STATE_WRITE_IN_VIEW]
                    env.storage().instance().set(&KEY, &1u32);
                    1
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert!(v.is_empty(), "explicit opt-out must be respected: {v:#?}");
    }

    #[test]
    fn flags_balance_of_suffix() {
        let src = r#"
            impl C {
                pub fn balance_of(env: Env, who: Address) -> i128 {
                    env.storage().persistent().remove(&who);
                    0
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("remove"));
    }

    #[test]
    fn ignores_pure_getter() {
        let src = r#"
            impl C {
                pub fn get_owner(env: Env) -> Address {
                    env.storage().instance().get(&KEY).unwrap()
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert!(v.is_empty());
    }

    #[test]
    fn skips_test_modules() {
        let src = r#"
            #[cfg(test)]
            mod tests {
                impl C {
                    pub fn get_balance(env: Env) -> i128 {
                        env.storage().persistent().set(&KEY, &0i128);
                        0
                    }
                }
            }
        "#;
        let v = StateWriteInViewRule::new().check(src);
        assert!(
            v.is_empty(),
            "writes inside #[cfg(test)] are ignored: {v:#?}"
        );
    }
}
