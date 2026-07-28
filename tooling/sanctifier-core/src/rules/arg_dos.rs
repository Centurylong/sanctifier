use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_ARG_DOS";

/// Detects contract entrypoints that iterate over Vec/Map arguments without a
/// visible length cap.
pub struct ArgDosRule;

impl ArgDosRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        fn_name: &str,
        visibility: &syn::Visibility,
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        if !matches!(visibility, syn::Visibility::Public(_)) {
            return Vec::new();
        }

        let arg_collections = collection_args(sig);
        if arg_collections.is_empty() {
            return Vec::new();
        }

        let mut visitor = ArgDosVisitor {
            arg_collections,
            capped_args: HashSet::new(),
            iterations: Vec::new(),
        };
        visitor.visit_block(block);

        visitor
            .iterations
            .into_iter()
            .filter(|iteration| !visitor.capped_args.contains(&iteration.arg_name))
            .map(|iteration| {
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    format!(
                        "{FINDING_CODE}: argument `{}` is iterated without a visible length cap",
                        iteration.arg_name
                    ),
                    format!("{}:{}", fn_name, iteration.line),
                )
                .with_suggestion(format!(
                    "Check `{0}.len()` against a maximum before iterating over `{0}`",
                    iteration.arg_name
                ))
            })
            .collect()
    }
}

impl Default for ArgDosRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ArgDosRule {
    fn name(&self) -> &str {
        "arg_dos"
    }

    fn description(&self) -> &str {
        "Detects Vec/Map arguments iterated without a visible length cap"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = ArgDosRuleVisitor {
            rule: self,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ArgDosRuleVisitor<'rule> {
    rule: &'rule ArgDosRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for ArgDosRuleVisitor<'_> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_item_fn(self, node);
    }
}

struct ArgDosVisitor {
    arg_collections: HashSet<String>,
    capped_args: HashSet<String>,
    iterations: Vec<ArgIteration>,
}

#[derive(Debug, Clone)]
struct ArgIteration {
    arg_name: String,
    line: usize,
}

impl<'ast> Visit<'ast> for ArgDosVisitor {
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.record_len_caps(&node.cond);
        syn::visit::visit_expr_if(self, node);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.record_len_caps(&node.cond);
        syn::visit::visit_expr_while(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if is_assert_or_guard_macro(node) {
            let tokens = node.tokens.to_string();
            for arg_name in &self.arg_collections {
                if tokens_mentions_len(&tokens, arg_name) {
                    self.capped_args.insert(arg_name.clone());
                }
            }
        }
        syn::visit::visit_macro(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        if let Some(arg_name) = iterated_collection_arg(&node.expr, &self.arg_collections) {
            self.iterations.push(ArgIteration {
                arg_name,
                line: node.for_token.span.start().line,
            });
        }
        syn::visit::visit_expr_for_loop(self, node);
    }
}

impl ArgDosVisitor {
    fn record_len_caps(&mut self, expr: &syn::Expr) {
        let mut visitor = LenCapVisitor {
            arg_collections: &self.arg_collections,
            capped_args: HashSet::new(),
        };
        visitor.visit_expr(expr);
        self.capped_args.extend(visitor.capped_args);
    }
}

struct LenCapVisitor<'args> {
    arg_collections: &'args HashSet<String>,
    capped_args: HashSet<String>,
}

impl<'ast> Visit<'ast> for LenCapVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "len" {
            if let Some(arg_name) = path_ident(&node.receiver) {
                if self.arg_collections.contains(&arg_name) {
                    self.capped_args.insert(arg_name);
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn collection_args(sig: &syn::Signature) -> HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_type) if is_vec_or_map_type(&pat_type.ty) => {
                pat_ident(&pat_type.pat)
            }
            _ => None,
        })
        .collect()
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

fn is_vec_or_map_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "Vec" | "Map")),
        syn::Type::Reference(reference) => is_vec_or_map_type(&reference.elem),
        _ => false,
    }
}

fn iterated_collection_arg(expr: &syn::Expr, arg_collections: &HashSet<String>) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method_call) if method_call.method == "iter" => {
            path_ident(&method_call.receiver).filter(|name| arg_collections.contains(name))
        }
        syn::Expr::Path(_) => path_ident(expr).filter(|name| arg_collections.contains(name)),
        _ => None,
    }
}

fn path_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Reference(reference) => path_ident(&reference.expr),
        _ => None,
    }
}

fn is_assert_or_guard_macro(node: &syn::Macro) -> bool {
    node.path.segments.last().is_some_and(|segment| {
        matches!(
            segment.ident.to_string().as_str(),
            "assert"
                | "assert_eq"
                | "assert_ne"
                | "debug_assert"
                | "debug_assert_eq"
                | "debug_assert_ne"
                | "ensure"
                | "require"
        )
    })
}

fn tokens_mentions_len(tokens: &str, arg_name: &str) -> bool {
    let compact: String = tokens.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.contains(&format!("{arg_name}.len()"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_uncapped_argument_vector_iteration() {
        let source = r#"
            use soroban_sdk::{Address, Env, Vec};

            impl Contract {
                pub fn airdrop(env: Env, recipients: Vec<Address>) {
                    for recipient in recipients.iter() {
                        touch(&env, recipient);
                    }
                }
            }
        "#;

        let findings = ArgDosRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].location.contains("airdrop"));
    }
}
