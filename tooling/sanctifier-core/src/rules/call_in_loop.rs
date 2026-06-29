use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_CALL_IN_LOOP";

/// Detects cross-contract calls performed inside loops.
pub struct CallInLoopRule;

impl CallInLoopRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CallInLoopRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for CallInLoopRule {
    fn name(&self) -> &str {
        FINDING_CODE
    }

    fn description(&self) -> &str {
        "Detects cross-contract invokes inside loop bodies"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = CallInLoopVisitor::default();
        visitor.visit_file(&file);

        visitor
            .issues
            .into_iter()
            .map(|issue| {
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    format!(
                        "{FINDING_CODE}: cross-contract call `{}` occurs inside a loop",
                        issue.call
                    ),
                    format!("line {}", issue.line),
                )
                .with_suggestion(
                    "Avoid push-style batch invokes in loops; record claims and let recipients pull payments individually."
                        .to_string(),
                )
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct CallInLoopVisitor {
    loop_depth: usize,
    issues: Vec<LoopCallIssue>,
}

struct LoopCallIssue {
    call: String,
    line: usize,
}

impl<'ast> Visit<'ast> for CallInLoopVisitor {
    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.loop_depth += 1;
        syn::visit::visit_expr_for_loop(self, node);
        self.loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loop_depth += 1;
        syn::visit::visit_expr_while(self, node);
        self.loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loop_depth += 1;
        syn::visit::visit_expr_loop(self, node);
        self.loop_depth -= 1;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.loop_depth > 0 && is_cross_contract_method_call(node) {
            self.issues.push(LoopCallIssue {
                call: method_call_to_string(node),
                line: node.span().start().line,
            });
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn is_cross_contract_method_call(node: &syn::ExprMethodCall) -> bool {
    let method = node.method.to_string();

    if method == "invoke_contract" || method == "try_invoke_contract" {
        return true;
    }

    if is_local_or_builder_method(&method) {
        return false;
    }

    receiver_name(&node.receiver).is_some_and(|receiver| is_external_receiver(&receiver))
}

fn is_external_receiver(receiver: &str) -> bool {
    receiver == "client"
        || receiver.ends_with("_client")
        || receiver.contains("client")
        || receiver == "token"
        || receiver.ends_with("_token")
        || receiver == "contract"
}

fn receiver_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Reference(reference) => receiver_name(&reference.expr),
        syn::Expr::Paren(paren) => receiver_name(&paren.expr),
        _ => None,
    }
}

fn is_local_or_builder_method(method: &str) -> bool {
    matches!(
        method,
        "iter"
            | "iter_mut"
            | "into_iter"
            | "enumerate"
            | "map"
            | "filter"
            | "fold"
            | "collect"
            | "len"
            | "is_empty"
            | "push"
            | "pop"
            | "get"
            | "set"
            | "insert"
            | "remove"
            | "clone"
            | "to_owned"
            | "unwrap"
            | "expect"
            | "unwrap_or"
            | "unwrap_or_else"
            | "require_auth"
    )
}

fn method_call_to_string(node: &syn::ExprMethodCall) -> String {
    let rendered = quote::quote!(#node).to_string();
    if rendered.len() > 100 {
        format!("{}...", &rendered[..97])
    } else {
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_client_call_inside_for_loop() {
        let source = r#"
            use soroban_sdk::{Address, Env, Vec};

            pub fn distribute(token: TokenClient, from: Address, recipients: Vec<Address>, amt: i128) {
                for recipient in recipients.iter() {
                    token.transfer(&from, &recipient, &amt);
                }
            }
        "#;

        let findings = CallInLoopRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].message.contains("transfer"));
        assert!(findings[0].suggestion.as_ref().unwrap().contains("pull"));
    }

    #[test]
    fn detects_env_invoke_contract_inside_while_loop() {
        let source = r#"
            pub fn call_many(env: Env, ids: Vec<Address>) {
                let mut i = 0;
                while i < ids.len() {
                    env.invoke_contract::<()>(&ids.get(i).unwrap(), &symbol_short!("ping"), vec![&env]);
                    i += 1;
                }
            }
        "#;

        let findings = CallInLoopRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("invoke_contract"));
    }

    #[test]
    fn ignores_batched_pull_accounting_without_external_call() {
        let source = r#"
            pub fn accrue(recipients: Vec<Address>, mut balances: Map<Address, i128>, amt: i128) {
                for recipient in recipients.iter() {
                    balances.set(recipient, amt);
                }
            }
        "#;

        let findings = CallInLoopRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn ignores_external_call_outside_loop() {
        let source = r#"
            pub fn withdraw(token: TokenClient, from: Address, to: Address, amt: i128) {
                token.transfer(&from, &to, &amt);
            }
        "#;

        let findings = CallInLoopRule::new().check(source);

        assert!(findings.is_empty());
    }
}
