use crate::finding_codes::SANCT_UNWRAP;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Flags panic-prone option/result handling in Soroban contract entrypoints.
pub struct SanctUnwrapRule;

impl SanctUnwrapRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SanctUnwrapRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for SanctUnwrapRule {
    fn name(&self) -> &str {
        "sanct_unwrap"
    }

    fn description(&self) -> &str {
        "Detects unwrap/expect/default fallbacks in #[contractimpl] entrypoints"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractUnwrapVisitor {
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

struct ContractUnwrapVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ContractUnwrapVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ContractUnwrapVisitor {
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
                    fn_returns_financial: returns_financial_type(&function.sig),
                    issues: Vec::new(),
                };
                function_visitor.visit_block(&function.block);

                self.violations.extend(
                    function_visitor
                        .issues
                        .into_iter()
                        .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                        .map(UnwrapIssue::into_violation),
                );
            }
        }
    }
}

struct ContractFunctionVisitor {
    fn_name: String,
    fn_returns_financial: bool,
    issues: Vec<UnwrapIssue>,
}

impl<'ast> Visit<'ast> for ContractFunctionVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let should_flag = match method.as_str() {
            "unwrap" | "expect" => true,
            "unwrap_or_default" => {
                self.fn_returns_financial
                    || receiver_looks_like_contract_storage_get(&node.receiver)
            }
            _ => false,
        };

        if should_flag {
            self.issues.push(UnwrapIssue {
                fn_name: self.fn_name.clone(),
                method,
                receiver: display_expr(&node.receiver),
                line: node.method.span().start().line,
            });
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

struct UnwrapIssue {
    fn_name: String,
    method: String,
    receiver: String,
    line: usize,
}

impl UnwrapIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            SANCT_UNWRAP,
            Severity::Warning,
            format!(
                "{SANCT_UNWRAP}: `{}` in contract entrypoint `{}` can abort transactions or hide missing state",
                self.method, self.fn_name
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(format!(
            "Replace `{}` on `{}` with typed Result handling, an explicit default, or a domain-specific Error",
            self.method, self.receiver
        ))
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

fn returns_financial_type(sig: &syn::Signature) -> bool {
    match &sig.output {
        syn::ReturnType::Type(_, ty) => type_path_contains_financial_name(ty),
        syn::ReturnType::Default => false,
    }
}

fn type_path_contains_financial_name(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.iter().any(|segment| {
            matches!(
                segment.ident.to_string().as_str(),
                "i128"
                    | "u128"
                    | "i64"
                    | "u64"
                    | "i32"
                    | "u32"
                    | "Amount"
                    | "Balance"
                    | "Balances"
                    | "Shares"
                    | "Supply"
            )
        }),
        syn::Type::Reference(reference) => type_path_contains_financial_name(&reference.elem),
        syn::Type::Tuple(tuple) => tuple.elems.iter().any(type_path_contains_financial_name),
        _ => false,
    }
}

fn receiver_looks_like_contract_storage_get(expr: &syn::Expr) -> bool {
    let receiver = display_expr(expr);
    let compact: String = receiver.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.contains(".storage().persistent().get")
        || compact.contains(".storage().instance().get")
        || compact.contains(".storage().temporary().get")
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
            line.contains("sanctifier:ignore[SANCT_UNWRAP]")
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
    fn detects_unwrap_only_inside_contractimpl_entrypoints() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn entry(env: Env) {
                    env.storage().instance().get(&"admin").unwrap();
                }
            }

            impl Contract {
                pub fn helper(env: Env) {
                    env.storage().instance().get(&"helper").unwrap();
                }
            }
        "#;

        let findings = SanctUnwrapRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, SANCT_UNWRAP);
        assert!(findings[0].location.contains("entry"));
    }

    #[test]
    fn skips_cfg_test_modules_and_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn suppressed(env: Env) {
                    // sanctifier:ignore[SANCT_UNWRAP]
                    env.storage().instance().get(&"admin").unwrap();
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[contractimpl]
                impl Contract {
                    pub fn test_entry(env: Env) {
                        env.storage().instance().get(&"test").unwrap();
                    }
                }
            }
        "#;

        let findings = SanctUnwrapRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
