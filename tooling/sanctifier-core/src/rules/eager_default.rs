use crate::finding_codes::SANCT_EAGER_UNWRAP_OR;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects `unwrap_or(...)` defaults that are evaluated eagerly.
pub struct EagerDefaultRule;

impl EagerDefaultRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EagerDefaultRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for EagerDefaultRule {
    fn name(&self) -> &str {
        "eager_default"
    }

    fn description(&self) -> &str {
        "Detects expensive defaults passed to unwrap_or and suggests unwrap_or_else"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = EagerDefaultVisitor {
            violations: Vec::new(),
            current_fn: None,
            public_depth: 0,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct EagerDefaultVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
    public_depth: usize,
}

impl EagerDefaultVisitor {
    fn enter_function(&mut self, name: String, visibility: &syn::Visibility) -> FunctionGuard {
        let previous_fn = self.current_fn.replace(name);
        let was_public = matches!(visibility, syn::Visibility::Public(_));
        if was_public {
            self.public_depth += 1;
        }

        FunctionGuard {
            previous_fn,
            was_public,
        }
    }

    fn leave_function(&mut self, guard: FunctionGuard) {
        if guard.was_public {
            self.public_depth -= 1;
        }
        self.current_fn = guard.previous_fn;
    }

    fn in_public_function(&self) -> bool {
        self.public_depth > 0
    }
}

struct FunctionGuard {
    previous_fn: Option<String>,
    was_public: bool,
}

impl<'ast> Visit<'ast> for EagerDefaultVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let guard = self.enter_function(node.sig.ident.to_string(), &node.vis);
        syn::visit::visit_impl_item_fn(self, node);
        self.leave_function(guard);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let guard = self.enter_function(node.sig.ident.to_string(), &node.vis);
        syn::visit::visit_item_fn(self, node);
        self.leave_function(guard);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.in_public_function() && node.method == "unwrap_or" {
            if let Some(default_expr) = node.args.first() {
                if is_expensive_default(default_expr) {
                    let fn_name = self
                        .current_fn
                        .clone()
                        .unwrap_or_else(|| "<unknown>".to_string());
                    let default_rendered = display_expr(default_expr);
                    self.violations.push(
                        RuleViolation::new(
                            SANCT_EAGER_UNWRAP_OR,
                            Severity::Warning,
                            format!(
                                "{SANCT_EAGER_UNWRAP_OR}: `unwrap_or` in `{fn_name}` eagerly evaluates `{default_rendered}` even when the option/result has a value"
                            ),
                            format!("{}:{}", fn_name, node.method.span().start().line),
                        )
                        .with_suggestion(format!(
                            "Use `unwrap_or_else(|| {default_rendered})` so the default is only computed on the fallback path"
                        )),
                    );
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

fn is_expensive_default(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Call(_)
        | syn::Expr::MethodCall(_)
        | syn::Expr::Macro(_)
        | syn::Expr::Block(_)
        | syn::Expr::If(_)
        | syn::Expr::Match(_)
        | syn::Expr::ForLoop(_)
        | syn::Expr::Loop(_)
        | syn::Expr::While(_)
        | syn::Expr::Closure(_)
        | syn::Expr::Async(_)
        | syn::Expr::Await(_)
        | syn::Expr::Try(_)
        | syn::Expr::Unsafe(_) => true,
        syn::Expr::Array(array) => array.elems.iter().any(is_expensive_default),
        syn::Expr::Assign(assign) => is_expensive_default(&assign.right),
        syn::Expr::Binary(binary) => {
            is_expensive_default(&binary.left) || is_expensive_default(&binary.right)
        }
        syn::Expr::Cast(cast) => is_expensive_default(&cast.expr),
        syn::Expr::Group(group) => is_expensive_default(&group.expr),
        syn::Expr::Index(index) => {
            is_expensive_default(&index.expr) || is_expensive_default(&index.index)
        }
        syn::Expr::Paren(paren) => is_expensive_default(&paren.expr),
        syn::Expr::Range(range) => {
            range
                .start
                .as_ref()
                .is_some_and(|expr| is_expensive_default(expr))
                || range
                    .end
                    .as_ref()
                    .is_some_and(|expr| is_expensive_default(expr))
        }
        syn::Expr::Reference(reference) => is_expensive_default(&reference.expr),
        syn::Expr::Repeat(repeat) => {
            is_expensive_default(&repeat.expr) || is_expensive_default(&repeat.len)
        }
        syn::Expr::Struct(struct_lit) => struct_lit
            .fields
            .iter()
            .any(|field| is_expensive_default(&field.expr)),
        syn::Expr::Tuple(tuple) => tuple.elems.iter().any(is_expensive_default),
        syn::Expr::Unary(unary) => is_expensive_default(&unary.expr),
        _ => false,
    }
}

fn display_expr(expr: &syn::Expr) -> String {
    let rendered = quote::quote!(#expr).to_string();
    if rendered.len() > 96 {
        format!("{}...", &rendered[..93])
    } else {
        rendered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_expensive_unwrap_or_defaults() {
        let source = r#"
            pub fn amount(primary: Option<i128>) -> i128 {
                primary.unwrap_or(compute_default())
            }

            fn compute_default() -> i128 {
                10
            }
        "#;

        let findings = EagerDefaultRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, SANCT_EAGER_UNWRAP_OR);
        assert!(findings[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("unwrap_or_else"));
    }

    #[test]
    fn leaves_cheap_defaults_and_private_helpers_alone() {
        let source = r#"
            pub fn cheap(primary: Option<i128>, fallback: i128) -> i128 {
                primary.unwrap_or(fallback)
            }

            fn private(primary: Option<i128>) -> i128 {
                primary.unwrap_or(compute_default())
            }

            fn compute_default() -> i128 {
                10
            }
        "#;

        let findings = EagerDefaultRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
