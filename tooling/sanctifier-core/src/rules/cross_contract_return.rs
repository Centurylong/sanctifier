use crate::finding_codes::CROSS_CONTRACT_RETURN;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects cross-contract calls whose return values are discarded.
pub struct CrossContractReturnRule;

impl CrossContractReturnRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CrossContractReturnRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for CrossContractReturnRule {
    fn name(&self) -> &str {
        "cross_contract_return"
    }

    fn description(&self) -> &str {
        "Detects unchecked return values from cross-contract calls"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractVisitor {
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

struct ContractVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
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
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) {
                    continue;
                }

                let mut function_visitor = ContractFunctionVisitor {
                    fn_name: function.sig.ident.to_string(),
                    issues: Vec::new(),
                };
                function_visitor.visit_block(&function.block);

                self.violations.extend(
                    function_visitor
                        .issues
                        .into_iter()
                        .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                        .map(CrossContractIssue::into_violation),
                );
            }
        }
    }
}

struct ContractFunctionVisitor {
    fn_name: String,
    issues: Vec<CrossContractIssue>,
}

impl<'ast> Visit<'ast> for ContractFunctionVisitor {
    fn visit_stmt(&mut self, node: &'ast syn::Stmt) {
        match node {
            syn::Stmt::Expr(expr, Some(_)) => {
                if let Some(call) = discarded_cross_contract_call(expr) {
                    self.issues.push(CrossContractIssue {
                        fn_name: self.fn_name.clone(),
                        call,
                        line: expr.span().start().line,
                    });
                }
            }
            syn::Stmt::Local(local) if is_ignored_binding(&local.pat) => {
                if let Some(init) = &local.init {
                    if let Some(call) = discarded_cross_contract_call(&init.expr) {
                        self.issues.push(CrossContractIssue {
                            fn_name: self.fn_name.clone(),
                            call,
                            line: local.span().start().line,
                        });
                    }
                }
            }
            _ => {}
        }

        syn::visit::visit_stmt(self, node);
    }
}

struct CrossContractIssue {
    fn_name: String,
    call: String,
    line: usize,
}

impl CrossContractIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            CROSS_CONTRACT_RETURN,
            Severity::Warning,
            format!(
                "{CROSS_CONTRACT_RETURN}: cross-contract call return value is discarded in `{}`",
                self.fn_name
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(format!(
            "Store, return, or explicitly validate the result of `{}` before continuing",
            self.call
        ))
    }
}

fn discarded_cross_contract_call(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method_call) if is_cross_contract_method_call(method_call) => {
            Some(display_expr(expr))
        }
        syn::Expr::Call(call) if is_invoke_contract_call(call) => Some(display_expr(expr)),
        syn::Expr::Paren(paren) => discarded_cross_contract_call(&paren.expr),
        syn::Expr::Group(group) => discarded_cross_contract_call(&group.expr),
        _ => None,
    }
}

fn is_cross_contract_method_call(method_call: &syn::ExprMethodCall) -> bool {
    method_call.method == "invoke_contract"
        || method_call.method == "try_invoke_contract"
        || receiver_looks_like_client(&method_call.receiver)
}

fn is_invoke_contract_call(call: &syn::ExprCall) -> bool {
    match &*call.func {
        syn::Expr::Path(path) => path.path.segments.last().is_some_and(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "invoke_contract" | "try_invoke_contract"
            )
        }),
        _ => false,
    }
}

fn receiver_looks_like_client(expr: &syn::Expr) -> bool {
    let rendered = display_expr(expr);
    let compact = rendered.to_ascii_lowercase();
    compact.ends_with("client") || compact.contains("_client") || compact.contains("client ::")
}

fn is_ignored_binding(pat: &syn::Pat) -> bool {
    match pat {
        syn::Pat::Wild(_) => true,
        syn::Pat::Ident(ident) => ident.ident.to_string().starts_with('_'),
        syn::Pat::Tuple(tuple) => tuple.elems.iter().all(is_ignored_binding),
        _ => false,
    }
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

fn display_expr(expr: &syn::Expr) -> String {
    let rendered = quote::quote!(#expr).to_string();
    if rendered.len() > 96 {
        format!("{}...", &rendered[..93])
    } else {
        rendered
    }
}

fn suppressions(source: &str) -> Vec<usize> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.contains("sanctifier:ignore[SANCT_CROSS_CONTRACT_RETURN]")
                .then_some(index + 1)
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions
        .iter()
        .any(|suppressed_line| *suppressed_line == line || *suppressed_line + 1 == line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_discarded_cross_contract_returns() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Symbol, Vec};

            #[contractimpl]
            impl Contract {
                pub fn unchecked(env: Env, token: Address, to: Address) {
                    let token_client = TokenClient::new(&env, &token);
                    token_client.transfer(&to, &to, &10);
                    env.invoke_contract::<i128>(&token, &Symbol::new(&env, "balance"), Vec::new(&env));
                    let _ = token_client.balance(&to);
                }
            }
        "#;

        let findings = CrossContractReturnRule::new().check(source);

        assert_eq!(findings.len(), 3, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|finding| finding.rule_name == CROSS_CONTRACT_RETURN));
    }

    #[test]
    fn allows_returned_bound_and_suppressed_calls() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Symbol, Vec};

            #[contractimpl]
            impl Contract {
                pub fn checked(env: Env, token: Address, to: Address) -> i128 {
                    let token_client = TokenClient::new(&env, &token);
                    let balance = token_client.balance(&to);
                    // sanctifier:ignore[SANCT_CROSS_CONTRACT_RETURN]
                    token_client.transfer(&to, &to, &10);
                    env.invoke_contract::<i128>(&token, &Symbol::new(&env, "balance"), Vec::new(&env))
                }
            }
        "#;

        let findings = CrossContractReturnRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
