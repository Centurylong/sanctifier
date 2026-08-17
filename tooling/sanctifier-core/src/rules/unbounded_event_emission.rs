use crate::finding_codes::UNBOUNDED_EVENT_EMISSION;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects `env.events().publish(..)` issued from inside a `for`/`while`/`loop`
/// body, where the number of published events (and the ledger's event-XDR
/// budget they consume) scales with the loop's iteration count instead of a
/// fixed, reviewed bound.
pub struct UnboundedEventEmissionRule;

impl UnboundedEventEmissionRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnboundedEventEmissionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UnboundedEventEmissionRule {
    fn name(&self) -> &str {
        "unbounded_event_emission"
    }

    fn description(&self) -> &str {
        "Detects env.events().publish(..) issued inside a loop without an iteration bound"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = LoopPublishVisitor {
            violations: Vec::new(),
            loop_depth: 0,
            current_fn: None,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct LoopPublishVisitor {
    violations: Vec<RuleViolation>,
    loop_depth: usize,
    current_fn: Option<String>,
}

impl LoopPublishVisitor {
    fn enter_loop<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.loop_depth += 1;
        f(self);
        self.loop_depth -= 1;
    }
}

impl<'ast> Visit<'ast> for LoopPublishVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let previous = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = previous;
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        syn::visit::visit_expr(self, &node.expr);
        self.enter_loop(|v| syn::visit::visit_block(v, &node.body));
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        syn::visit::visit_expr(self, &node.cond);
        self.enter_loop(|v| syn::visit::visit_block(v, &node.body));
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.enter_loop(|v| syn::visit::visit_block(v, &node.body));
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.loop_depth > 0 && node.method == "publish" && events_chain(&node.receiver) {
            if let Some(fn_name) = &self.current_fn {
                self.violations.push(
                    RuleViolation::new(
                        UNBOUNDED_EVENT_EMISSION,
                        Severity::Warning,
                        format!(
                            "{UNBOUNDED_EVENT_EMISSION}: `{fn_name}` publishes an event from inside a loop, so the event count (and its XDR/budget cost) scales with the loop's iteration count"
                        ),
                        format!("{fn_name}:{}", node.method.span().start().line),
                    )
                    .with_suggestion(
                        "Emit a single summary event after the loop, or cap the number of iterations that can reach `publish`".to_string(),
                    ),
                );
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// True when the receiver resolves through `.events()`, i.e. `env.events()...`.
fn events_chain(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(call) if call.method == "events" => true,
        syn::Expr::MethodCall(call) => events_chain(&call.receiver),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_publish_in_for_loop() {
        let source = r#"
            impl Contract {
                pub fn notify_all(env: Env, recipients: Vec<Address>) {
                    for to in recipients.iter() {
                        env.events().publish((symbol_short!("sent"), to.clone()), amount);
                    }
                }
            }
        "#;

        let findings = UnboundedEventEmissionRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].location.contains("notify_all"));
    }

    #[test]
    fn flags_publish_in_while_loop() {
        let source = r#"
            impl Contract {
                pub fn drain_queue(env: Env, mut queue: Vec<Address>) {
                    while let Some(to) = queue.pop_front() {
                        env.events().publish((symbol_short!("drain"),), to);
                    }
                }
            }
        "#;

        let findings = UnboundedEventEmissionRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
    }

    #[test]
    fn ignores_publish_outside_loop() {
        let source = r#"
            impl Contract {
                pub fn notify_one(env: Env, to: Address) {
                    env.events().publish((symbol_short!("sent"), to), amount);
                }
            }
        "#;

        let findings = UnboundedEventEmissionRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_unrelated_call_in_loop() {
        let source = r#"
            impl Contract {
                pub fn sum(env: Env, values: Vec<i128>) -> i128 {
                    let mut total = 0;
                    for v in values.iter() {
                        total += v;
                    }
                    total
                }
            }
        "#;

        let findings = UnboundedEventEmissionRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
