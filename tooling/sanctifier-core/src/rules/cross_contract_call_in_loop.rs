use crate::finding_codes::CROSS_CONTRACT_CALL_IN_LOOP;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects a cross-contract invocation (`env.invoke_contract`, or a call through
/// a generated `*Client`) issued from inside a `for`/`while`/`loop` body.
///
/// Each cross-contract call is a separate host-to-host invocation with its own
/// CPU/memory budget accounting; issuing one per loop iteration over
/// caller-influenced input lets a single transaction's cost scale with an
/// unbounded collection, risking a resource-limit abort (or, pre-abort, a
/// denial-of-service on legitimate callers sharing the ledger's budget).
pub struct CrossContractCallInLoopRule;

impl CrossContractCallInLoopRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CrossContractCallInLoopRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for CrossContractCallInLoopRule {
    fn name(&self) -> &str {
        "cross_contract_call_in_loop"
    }

    fn description(&self) -> &str {
        "Detects cross-contract calls (env.invoke_contract or a *Client) issued inside a loop"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = LoopCallVisitor {
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

struct LoopCallVisitor {
    violations: Vec<RuleViolation>,
    loop_depth: usize,
    current_fn: Option<String>,
}

impl LoopCallVisitor {
    fn enter_loop<F: FnOnce(&mut Self)>(&mut self, f: F) {
        self.loop_depth += 1;
        f(self);
        self.loop_depth -= 1;
    }
}

impl<'ast> Visit<'ast> for LoopCallVisitor {
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
        if self.loop_depth > 0 {
            let method = node.method.to_string();
            let is_cross_contract = method == "invoke_contract"
                || (client_call(&node.receiver) && is_likely_contract_method(&method));

            if is_cross_contract {
                if let Some(fn_name) = &self.current_fn {
                    self.violations.push(
                        RuleViolation::new(
                            CROSS_CONTRACT_CALL_IN_LOOP,
                            Severity::Warning,
                            format!(
                                "{CROSS_CONTRACT_CALL_IN_LOOP}: `{fn_name}` issues a cross-contract call (`{method}`) from inside a loop, so its cost scales with the loop's iteration count"
                            ),
                            format!("{fn_name}:{}", node.method.span().start().line),
                        )
                        .with_suggestion(
                            "Batch the cross-contract call outside the loop, or cap the number of iterations that can reach it".to_string(),
                        ),
                    );
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// True when `expr` is (or chains from) a call to `<Ident>Client::new(..)`, the
/// generated client Soroban emits for cross-contract invocation.
fn client_call(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(call) => match &*call.func {
            syn::Expr::Path(path) => path
                .path
                .segments
                .iter()
                .any(|seg| seg.ident == "new" || seg.ident.to_string().ends_with("Client")),
            _ => false,
        },
        syn::Expr::MethodCall(call) => client_call(&call.receiver),
        _ => false,
    }
}

/// Excludes plain builder/getter noise (`new`, `clone`, ...) so only the actual
/// contract-method hop on a client is flagged.
fn is_likely_contract_method(method: &str) -> bool {
    !matches!(method, "new" | "clone" | "address" | "clone_from")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_invoke_contract_in_for_loop() {
        let source = r#"
            impl Contract {
                pub fn broadcast(env: Env, targets: Vec<Address>) {
                    for target in targets.iter() {
                        env.invoke_contract::<()>(&target, &Symbol::new(&env, "ping"), Vec::new(&env));
                    }
                }
            }
        "#;

        let findings = CrossContractCallInLoopRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].location.contains("broadcast"));
    }

    #[test]
    fn flags_client_call_in_while_loop() {
        let source = r#"
            impl Contract {
                pub fn drain(env: Env, mut recipients: Vec<Address>) {
                    while let Some(to) = recipients.pop_front() {
                        TokenClient::new(&env, &token).transfer(&env.current_contract_address(), &to, &amount);
                    }
                }
            }
        "#;

        let findings = CrossContractCallInLoopRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("transfer"));
    }

    #[test]
    fn ignores_cross_contract_call_outside_loop() {
        let source = r#"
            impl Contract {
                pub fn ping_once(env: Env, target: Address) {
                    env.invoke_contract::<()>(&target, &Symbol::new(&env, "ping"), Vec::new(&env));
                }
            }
        "#;

        let findings = CrossContractCallInLoopRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_plain_loop_body_without_cross_contract_call() {
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

        let findings = CrossContractCallInLoopRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
