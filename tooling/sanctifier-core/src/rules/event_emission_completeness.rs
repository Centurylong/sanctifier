use crate::finding_codes::EVENT_EMISSION_GAP;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Attribute, Meta};

/// Detects gaps in an established Soroban contract event surface.
///
/// The rule is intentionally conservative: it only reports a public
/// `#[contractimpl]` entrypoint when the same impl already publishes at least
/// one event elsewhere, the entrypoint performs a direct Soroban storage
/// `set`/`update`/`remove`, and that entrypoint publishes no event itself.
/// Contracts with no event surface are left alone rather than assuming that
/// every storage write must be externally indexed.
pub struct EventEmissionCompletenessRule;

impl EventEmissionCompletenessRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EventEmissionCompletenessRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for EventEmissionCompletenessRule {
    fn name(&self) -> &str {
        "event_emission_completeness"
    }

    fn description(&self) -> &str {
        "Detects state-mutating entrypoints that omit events in contracts with an established event surface"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };

        let mut visitor = EventCompletenessVisitor {
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

struct EventCompletenessVisitor {
    violations: Vec<RuleViolation>,
    test_depth: usize,
}

impl EventCompletenessVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for EventCompletenessVisitor {
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
        if self.in_test_module() || !has_contractimpl(&node.attrs) {
            return;
        }

        let mut entrypoints = Vec::new();
        for item in &node.items {
            let syn::ImplItem::Fn(function) = item else {
                continue;
            };
            if !matches!(function.vis, syn::Visibility::Public(_)) {
                continue;
            }

            let mut effects = FunctionEffectVisitor {
                writes: Vec::new(),
                publishes_event: false,
            };
            effects.visit_block(&function.block);
            entrypoints.push((function, effects));
        }

        if !entrypoints.iter().any(|(_, effects)| effects.publishes_event) {
            return;
        }

        for (function, effects) in entrypoints {
            if effects.publishes_event {
                continue;
            }
            if let Some(first_write) = effects.writes.first() {
                let fn_name = function.sig.ident.to_string();
                self.violations.push(
                    RuleViolation::new(
                        EVENT_EMISSION_GAP,
                        Severity::Warning,
                        format!(
                            "{EVENT_EMISSION_GAP}: state-mutating entrypoint `{fn_name}` performs a storage `{}` write but publishes no event, while sibling entrypoints establish an event surface for this contract",
                            first_write.method
                        ),
                        format!("{fn_name}:{}", first_write.line),
                    )
                    .with_suggestion(
                        "Publish a compact event for the externally observable state transition so off-chain indexers see it consistently with the contract's existing event surface."
                            .to_string(),
                    ),
                );
            }
        }
    }
}

struct FunctionEffectVisitor {
    writes: Vec<WriteHit>,
    publishes_event: bool,
}

struct WriteHit {
    method: String,
    line: usize,
}

impl<'ast> Visit<'ast> for FunctionEffectVisitor {
    fn visit_item_fn(&mut self, _node: &'ast syn::ItemFn) {}

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if matches!(method.as_str(), "set" | "update" | "remove") && is_storage_receiver(&node.receiver) {
            self.writes.push(WriteHit {
                method,
                line: node.method.span().start().line,
            });
        }
        if node.method == "publish" && events_chain(&node.receiver) {
            self.publishes_event = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn is_storage_receiver(receiver: &syn::Expr) -> bool {
    match receiver {
        syn::Expr::MethodCall(call) if call.method == "storage" => true,
        syn::Expr::MethodCall(call)
            if matches!(
                call.method.to_string().as_str(),
                "persistent" | "temporary" | "instance"
            ) =>
        {
            is_storage_receiver(&call.receiver)
        }
        syn::Expr::Paren(paren) => is_storage_receiver(&paren.expr),
        syn::Expr::Group(group) => is_storage_receiver(&group.expr),
        _ => false,
    }
}

fn events_chain(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(call) if call.method == "events" => true,
        syn::Expr::MethodCall(call) => events_chain(&call.receiver),
        _ => false,
    }
}

fn has_contractimpl(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("contractimpl"))
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        let Meta::List(list) = &attr.meta else {
            return false;
        };

        let parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
        parser
            .parse2(list.tokens.clone())
            .map(|items| items.iter().any(|meta| cfg_meta_has_positive_test(meta, false)))
            .unwrap_or(false)
    })
}

fn cfg_meta_has_positive_test(meta: &Meta, negated: bool) -> bool {
    match meta {
        Meta::Path(path) => !negated && path.is_ident("test"),
        Meta::List(list) => {
            let nested_negated = if list.path.is_ident("not") {
                !negated
            } else {
                negated
            };
            let parser = Punctuated::<Meta, syn::Token![,]>::parse_terminated;
            parser
                .parse2(list.tokens.clone())
                .map(|items| {
                    items
                        .iter()
                        .any(|meta| cfg_meta_has_positive_test(meta, nested_negated))
                })
                .unwrap_or(false)
        }
        Meta::NameValue(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_suppress_production_code_behind_cfg_not_test() {
        let source = r#"
            #[cfg(not(test))]
            mod production {
                #[contractimpl]
                impl Contract {
                    pub fn emit(env: Env) {
                        env.events().publish(("changed",), ());
                    }

                    pub fn write(env: Env) {
                        env.storage().instance().set(&"value", &1_u32);
                    }
                }
            }
        "#;

        let findings = EventEmissionCompletenessRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].location.starts_with("write:"));
    }

    #[test]
    fn ignores_non_soroban_receivers_with_storage_like_names() {
        let source = r#"
            #[contractimpl]
            impl Contract {
                pub fn emit(env: Env) {
                    env.events().publish((Symbol::new(&env, "changed"),), ());
                }

                pub fn cache_only(instance_cache: Cache, storage_adapter: Cache) {
                    instance_cache.set("a", 1);
                    storage_adapter.update("b", 2);
                }
            }
        "#;

        assert!(EventEmissionCompletenessRule::new().check(source).is_empty());
    }
}
