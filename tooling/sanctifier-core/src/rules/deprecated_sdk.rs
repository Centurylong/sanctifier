use crate::finding_codes::DEPRECATED_SDK;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;

const FINDING_CODE: &str = DEPRECATED_SDK;

#[derive(Clone, Copy)]
enum ApiShape {
    EnvMethod(&'static str),
    EnvAccessorMethod {
        accessor: &'static str,
        method: &'static str,
    },
    AssociatedFn {
        ty: &'static str,
        function: &'static str,
    },
    Macro(&'static str),
}

#[derive(Clone, Copy)]
struct DeprecatedApi {
    display: &'static str,
    replacement: &'static str,
    shape: ApiShape,
}

// Maintained against callable and macro deprecations in the workspace
// soroban-sdk baseline (20.5.0). Deprecated modules/type aliases are outside
// this call-oriented rule's scope. Refresh the table when the SDK advances.
const DEPRECATED_APIS: &[DeprecatedApi] = &[
    DeprecatedApi {
        display: "Env::logger()",
        replacement: "Env::logs()",
        shape: ApiShape::EnvMethod("logger"),
    },
    DeprecatedApi {
        display: "Logs::log(..)",
        replacement: "Logs::add(..) (or the log! macro)",
        shape: ApiShape::EnvAccessorMethod {
            accessor: "logs",
            method: "log",
        },
    },
    DeprecatedApi {
        display: "Prng::u64_in_range(..)",
        replacement: "Prng::gen_range(..)",
        shape: ApiShape::EnvAccessorMethod {
            accessor: "prng",
            method: "u64_in_range",
        },
    },
    DeprecatedApi {
        display: "Symbol::short(..)",
        replacement: "the symbol_short! macro",
        shape: ApiShape::AssociatedFn {
            ty: "Symbol",
            function: "short",
        },
    },
    DeprecatedApi {
        display: "String::from_slice(..)",
        replacement: "String::from_str(..)",
        shape: ApiShape::AssociatedFn {
            ty: "String",
            function: "from_slice",
        },
    },
    DeprecatedApi {
        display: "panic_error!(..)",
        replacement: "panic_with_error!(..)",
        shape: ApiShape::Macro("panic_error"),
    },
];

pub struct DeprecatedSdkRule;

impl DeprecatedSdkRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DeprecatedSdkRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DeprecatedSdkRule {
    fn name(&self) -> &str {
        "deprecated_sdk"
    }

    fn description(&self) -> &str {
        "Detects known-deprecated soroban_sdk API calls and points to supported replacements"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(f) => (*f).clone(),
            None => return vec![],
        };
        let mut visitor = DeprecatedSdkVisitor {
            fn_name: String::new(),
            env_bindings: HashSet::new(),
            seen: HashSet::new(),
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct DeprecatedSdkVisitor {
    fn_name: String,
    env_bindings: HashSet<String>,
    seen: HashSet<(usize, usize, &'static str)>,
    violations: Vec<RuleViolation>,
}

impl DeprecatedSdkVisitor {
    fn record(&mut self, api: &'static DeprecatedApi, line: usize, column: usize) {
        // Independent deprecated calls may share one source line (including
        // generated Rust). The source column distinguishes these occurrences.
        if !self.seen.insert((line, column, api.display)) {
            return;
        }
        self.violations.push(
            RuleViolation::new(
                FINDING_CODE,
                Severity::Warning,
                format!(
                    "{FINDING_CODE}: deprecated soroban_sdk API '{}' is still in use",
                    api.display
                ),
                format!("{}:{}", self.fn_name, line),
            )
            .with_suggestion(format!("Use {} instead.", api.replacement)),
        );
    }
}

impl<'ast> Visit<'ast> for DeprecatedSdkVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev = std::mem::replace(&mut self.fn_name, node.sig.ident.to_string());
        let prev_env_bindings = std::mem::take(&mut self.env_bindings);
        collect_env_bindings(&node.sig, &mut self.env_bindings);
        syn::visit::visit_impl_item_fn(self, node);
        self.env_bindings = prev_env_bindings;
        self.fn_name = prev;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev = std::mem::replace(&mut self.fn_name, node.sig.ident.to_string());
        let prev_env_bindings = std::mem::take(&mut self.env_bindings);
        collect_env_bindings(&node.sig, &mut self.env_bindings);
        syn::visit::visit_item_fn(self, node);
        self.env_bindings = prev_env_bindings;
        self.fn_name = prev;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        for api in DEPRECATED_APIS {
            let matched = match api.shape {
                ApiShape::EnvMethod(method) => {
                    node.method == method && receiver_is_env(&node.receiver, &self.env_bindings)
                }
                ApiShape::EnvAccessorMethod { accessor, method } => {
                    node.method == method
                        && receiver_is_env_accessor(&node.receiver, accessor, &self.env_bindings)
                }
                ApiShape::AssociatedFn { .. } | ApiShape::Macro(_) => false,
            };
            if matched {
                self.record(api, node.span().start().line, node.span().start().column);
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            for api in DEPRECATED_APIS {
                if let ApiShape::AssociatedFn { ty, function } = api.shape {
                    if path_ends_with(&path.path, ty, function) {
                        self.record(api, node.span().start().line, node.span().start().column);
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(name) = node.path.segments.last().map(|segment| segment.ident.to_string()) {
            for api in DEPRECATED_APIS {
                if let ApiShape::Macro(macro_name) = api.shape {
                    if name == macro_name {
                        self.record(api, node.span().start().line, node.span().start().column);
                    }
                }
            }
        }
        syn::visit::visit_macro(self, node);
    }
}

fn collect_env_bindings(sig: &syn::Signature, bindings: &mut HashSet<String>) {
    for input in &sig.inputs {
        let syn::FnArg::Typed(arg) = input else {
            continue;
        };
        if !type_is_env(&arg.ty) {
            continue;
        }
        if let syn::Pat::Ident(pat) = &*arg.pat {
            bindings.insert(pat.ident.to_string());
        }
    }
}

fn type_is_env(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident == "Env")
            .unwrap_or(false),
        syn::Type::Reference(reference) => type_is_env(&reference.elem),
        syn::Type::Paren(paren) => type_is_env(&paren.elem),
        syn::Type::Group(group) => type_is_env(&group.elem),
        _ => false,
    }
}

fn receiver_is_env(expr: &syn::Expr, env_bindings: &HashSet<String>) -> bool {
    match expr {
        syn::Expr::Path(p) => p
            .path
            .get_ident()
            .map(|ident| ident == "env" || env_bindings.contains(&ident.to_string()))
            .unwrap_or(false),
        syn::Expr::Field(f) => match &f.member {
            syn::Member::Named(name) => name == "env",
            syn::Member::Unnamed(_) => false,
        },
        syn::Expr::Paren(p) => receiver_is_env(&p.expr, env_bindings),
        _ => false,
    }
}

fn receiver_is_env_accessor(
    expr: &syn::Expr,
    accessor: &str,
    env_bindings: &HashSet<String>,
) -> bool {
    match expr {
        syn::Expr::MethodCall(call) => {
            call.method == accessor
                && call.args.is_empty()
                && receiver_is_env(&call.receiver, env_bindings)
        }
        syn::Expr::Paren(p) => receiver_is_env_accessor(&p.expr, accessor, env_bindings),
        _ => false,
    }
}

fn path_ends_with(path: &syn::Path, ty: &str, function: &str) -> bool {
    let mut segments = path.segments.iter().rev();
    matches!(
        (segments.next(), segments.next()),
        (Some(last), Some(prev)) if last.ident == function && prev.ident == ty
    )
}


#[cfg(test)]
mod issue_682_same_line_regression {
    use super::*;

    #[test]
    fn distinct_deprecated_calls_on_one_line_are_not_collapsed() {
        let source = "pub fn repeated(env: Env) { env.logger(); env.logger(); }";
        let findings = DeprecatedSdkRule::new().check(source);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|v| v.rule_name == FINDING_CODE));
        assert!(findings.iter().all(|v| v.location.starts_with("repeated:")));
    }

    #[test]
    fn distinct_deprecated_macros_on_one_line_are_not_collapsed() {
        let source = "pub fn repeated(env: Env) { panic_error!(&env, E::A); panic_error!(&env, E::B); }";
        let findings = DeprecatedSdkRule::new().check(source);
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|v| v.rule_name == FINDING_CODE));
    }
}
