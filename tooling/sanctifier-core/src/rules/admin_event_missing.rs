use crate::finding_codes::SANCT_ADMIN_EVENT_MISSING;
use crate::rules::{Rule, RuleViolation, Severity};
#[allow(unused_imports)]
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects admin/config-change functions that mutate storage without emitting an event.
///
/// Soroban off-chain observers (indexers, UIs, monitoring systems) rely on events to
/// track privileged state changes. An admin function that silently updates storage
/// without an event emit leaves those consumers blind to the change.
///
/// A suppression comment `// sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]` on or
/// immediately before the function signature opts out explicitly.
pub struct AdminEventMissingRule;

impl AdminEventMissingRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdminEventMissingRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for AdminEventMissingRule {
    fn name(&self) -> &str {
        "admin_event_missing"
    }

    fn description(&self) -> &str {
        "Detects admin/config-change functions that mutate storage without emitting a corresponding on-chain event"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = AdminEventVisitor {
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

// ── Name heuristic ────────────────────────────────────────────────────────────

/// Returns true when the lowercase name contains any admin/config-change keyword.
fn is_admin_fn_name(name: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "set_",
        "update_",
        "change_",
        "configure",
        "pause",
        "unpause",
        "upgrade",
        "transfer_admin",
        "set_admin",
        "set_owner",
        "migrate",
    ];
    let lower = name.to_lowercase();
    KEYWORDS.iter().any(|kw| lower.contains(kw))
}

// ── Storage-receiver detection ────────────────────────────────────────────────

/// True when the receiver chain of a method call touches a Soroban storage handle.
fn is_storage_receiver(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(mc) => {
            let m = mc.method.to_string();
            m == "persistent"
                || m == "instance"
                || m == "temporary"
                || is_storage_receiver(&mc.receiver)
        }
        _ => false,
    }
}

// ── Event-chain detection ─────────────────────────────────────────────────────

/// True when any method call in the receiver chain is named `events`.
fn chain_contains_events(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(mc) => mc.method == "events" || chain_contains_events(&mc.receiver),
        _ => false,
    }
}

// ── Function body visitor ─────────────────────────────────────────────────────

/// Scans a single function body for storage mutations and event emits.
struct FunctionBodyVisitor {
    has_mutation: bool,
    has_event: bool,
}

impl<'ast> Visit<'ast> for FunctionBodyVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        // Storage mutation: set/update/remove on a storage receiver chain.
        if matches!(method.as_str(), "set" | "update" | "remove")
            && is_storage_receiver(&node.receiver)
        {
            self.has_mutation = true;
        }

        // Event emit: direct publish/emit/log call, or any call on an `.events()` chain.
        if matches!(method.as_str(), "publish" | "emit" | "log" | "events")
            || chain_contains_events(&node.receiver)
        {
            self.has_event = true;
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

// ── File-level visitor ────────────────────────────────────────────────────────

struct AdminEventVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl AdminEventVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for AdminEventVisitor {
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

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if self.in_test_module() {
            syn::visit::visit_impl_item_fn(self, node);
            return;
        }

        let fn_name = node.sig.ident.to_string();

        if !is_admin_fn_name(&fn_name) {
            syn::visit::visit_impl_item_fn(self, node);
            return;
        }

        let mut body_visitor = FunctionBodyVisitor {
            has_mutation: false,
            has_event: false,
        };
        body_visitor.visit_block(&node.block);

        if !body_visitor.has_mutation || body_visitor.has_event {
            syn::visit::visit_impl_item_fn(self, node);
            return;
        }

        let fn_line = node.sig.ident.span().start().line;

        if is_suppressed(&self.suppressions, fn_line) {
            syn::visit::visit_impl_item_fn(self, node);
            return;
        }

        self.violations.push(
            RuleViolation::new(
                SANCT_ADMIN_EVENT_MISSING,
                Severity::Warning,
                format!(
                    "{SANCT_ADMIN_EVENT_MISSING}: admin/config function `{fn_name}` mutates storage without emitting an event; add env.events().publish(...) to notify off-chain observers of the privileged state change"
                ),
                format!("{fn_name}:{fn_line}"),
            )
            .with_suggestion(
                "Add an event emit (e.g. env.events().publish((symbol_short!(\"admin\"), symbol_short!(\"<action>\")), value)) before or after the storage mutation. If this function intentionally omits events, annotate it with `// sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]`.".to_string(),
            ),
        );

        syn::visit::visit_impl_item_fn(self, node);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
            line.contains("sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]")
                .then_some(index + 1)
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions.iter().any(|s| *s == line || *s + 1 == line)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_admin_fn_with_storage_write_and_no_event() {
        let src = r#"
            impl Contract {
                pub fn set_admin(env: Env, new_admin: Address) {
                    env.storage().instance().set(&DataKey::Admin, &new_admin);
                }
            }
        "#;
        let violations = AdminEventMissingRule::new().check(src);
        assert_eq!(
            violations.len(),
            1,
            "expected 1 violation, got: {violations:#?}"
        );
        assert_eq!(violations[0].rule_name, SANCT_ADMIN_EVENT_MISSING);
        assert!(violations[0].message.contains("set_admin"));
    }

    #[test]
    fn does_not_flag_when_event_emit_present() {
        let src = r#"
            impl Contract {
                pub fn set_admin(env: Env, new_admin: Address) {
                    env.storage().instance().set(&DataKey::Admin, &new_admin);
                    env.events().publish((symbol_short!("admin"), symbol_short!("set")), new_admin);
                }
            }
        "#;
        let violations = AdminEventMissingRule::new().check(src);
        assert!(
            violations.is_empty(),
            "expected no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn does_not_flag_when_no_storage_mutation() {
        let src = r#"
            impl Contract {
                pub fn set_admin(env: Env) -> Address {
                    env.storage().instance().get(&DataKey::Admin).unwrap()
                }
            }
        "#;
        let violations = AdminEventMissingRule::new().check(src);
        assert!(
            violations.is_empty(),
            "expected no violations, got: {violations:#?}"
        );
    }

    #[test]
    fn respects_inline_suppression() {
        let src = r#"
            impl Contract {
                // sanctifier:ignore[SANCT_ADMIN_EVENT_MISSING]
                pub fn set_admin(env: Env, new_admin: Address) {
                    env.storage().instance().set(&DataKey::Admin, &new_admin);
                }
            }
        "#;
        let violations = AdminEventMissingRule::new().check(src);
        assert!(
            violations.is_empty(),
            "suppression should prevent violation: {violations:#?}"
        );
    }

    #[test]
    fn skips_cfg_test_modules() {
        let src = r#"
            #[cfg(test)]
            mod tests {
                impl Contract {
                    pub fn set_admin(env: Env, new_admin: Address) {
                        env.storage().instance().set(&DataKey::Admin, &new_admin);
                    }
                }
            }
        "#;
        let violations = AdminEventMissingRule::new().check(src);
        assert!(
            violations.is_empty(),
            "functions inside #[cfg(test)] should be skipped: {violations:#?}"
        );
    }
}
