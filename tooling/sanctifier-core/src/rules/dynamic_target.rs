use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{parse_str, File, FnArg, Pat, Type, Expr, BinOp, ExprBinary, ExprMethodCall, Macro, ImplItemFn, ItemFn};

const FINDING_CODE: &str = "SANCT_DYNAMIC_TARGET";

/// Detects contract invocations where the target address flows from untrusted function inputs
/// without an allowlist check or validation.
pub struct DynamicTargetRule;

impl DynamicTargetRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DynamicTargetRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DynamicTargetRule {
    fn name(&self) -> &str {
        "dynamic_target"
    }

    fn description(&self) -> &str {
        "Detects invoke targets whose Address/contract id flows from user arguments without validation"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = DynamicTargetVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);

        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct DynamicTargetVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for DynamicTargetVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        // Only analyze public entrypoints
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.analyze_function(&node.sig, &node.block);
        }
        visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        // Only analyze public entrypoints
        if matches!(node.vis, syn::Visibility::Public(_)) {
            self.analyze_function(&node.sig, &node.block);
        }
        visit::visit_item_fn(self, node);
    }
}

impl DynamicTargetVisitor {
    fn analyze_function(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let mut untrusted_vars = HashSet::new();
        let mut alias_to_root: HashMap<String, HashSet<String>> = HashMap::new();

        // 1. Collect function parameters of type Address
        for arg in &sig.inputs {
            if let FnArg::Typed(pat_type) = arg {
                if is_address_type(&pat_type.ty) {
                    if let Pat::Ident(pat_ident) = &*pat_type.pat {
                        let param_name = pat_ident.ident.to_string();
                        untrusted_vars.insert(param_name.clone());
                        let mut roots = HashSet::new();
                        roots.insert(param_name.clone());
                        alias_to_root.insert(param_name, roots);
                    }
                }
            }
        }

        // If there are no Address parameters, there can be no dynamic target vulnerability in this context
        if untrusted_vars.is_empty() {
            return;
        }

        // 2. Traversal over function block to find variables, validations, and sinks
        let mut scope_visitor = FunctionScopeVisitor {
            untrusted_vars,
            alias_to_root,
            validated_roots: HashSet::new(),
            violations: Vec::new(),
            fn_name: sig.ident.to_string(),
        };
        scope_visitor.visit_block(block);

        self.violations.extend(scope_visitor.violations);
    }
}

struct FunctionScopeVisitor {
    untrusted_vars: HashSet<String>,
    alias_to_root: HashMap<String, HashSet<String>>,
    validated_roots: HashSet<String>,
    violations: Vec<RuleViolation>,
    fn_name: String,
}

impl FunctionScopeVisitor {
    fn check_expr_for_untrusted(&self, expr: &Expr) -> Option<HashSet<String>> {
        let mut collector = VarCollector {
            untrusted_vars: &self.untrusted_vars,
            found_vars: Vec::new(),
        };
        collector.visit_expr(expr);
        if collector.found_vars.is_empty() {
            None
        } else {
            let mut roots = HashSet::new();
            for v in collector.found_vars {
                if let Some(r) = self.alias_to_root.get(&v) {
                    roots.extend(r.clone());
                }
            }
            Some(roots)
        }
    }

    fn mark_validated(&mut self, expr: &Expr) {
        if let Some(roots) = self.check_expr_for_untrusted(expr) {
            self.validated_roots.extend(roots);
        }
    }
}

impl<'ast> Visit<'ast> for FunctionScopeVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Pat::Ident(pat_ident) = &node.pat {
            let var_name = pat_ident.ident.to_string();
            if let Some(init) = &node.init {
                if let Some(roots) = self.check_expr_for_untrusted(&init.expr) {
                    self.untrusted_vars.insert(var_name.clone());
                    self.alias_to_root.insert(var_name, roots);
                }
            }
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if let Expr::Path(p) = &*node.left {
            if let Some(ident) = p.path.get_ident() {
                let var_name = ident.to_string();
                if let Some(roots) = self.check_expr_for_untrusted(&node.right) {
                    self.untrusted_vars.insert(var_name.clone());
                    self.alias_to_root.insert(var_name, roots);
                }
            }
        }
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast ExprBinary) {
        if matches!(node.op, BinOp::Eq(_) | BinOp::Ne(_)) {
            // If either side references an untrusted var, we validate both sides
            let has_left = self.check_expr_for_untrusted(&node.left).is_some();
            let has_right = self.check_expr_for_untrusted(&node.right).is_some();
            if has_left || has_right {
                self.mark_validated(&node.left);
                self.mark_validated(&node.right);
            }
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let method_name = node.method.to_string();
        if method_name == "contains" || method_name == "get" || method_name == "has" {
            // allowlist check: contains/get/has method parameters or receiver
            for arg in &node.args {
                self.mark_validated(arg);
            }
            self.mark_validated(&node.receiver);
        }

        // Direct invoke call: env.invoke_contract(contract_id, ...)
        if method_name == "invoke_contract" {
            if let Some(target_expr) = node.args.first() {
                if let Some(roots) = self.check_expr_for_untrusted(target_expr) {
                    // Check if any of the roots are not validated
                    let unvalidated: Vec<_> = roots
                        .iter()
                        .filter(|r| !self.validated_roots.contains(*r))
                        .cloned()
                        .collect();
                    if !unvalidated.is_empty() {
                        let line = node.span().start().line;
                        let var_ref = quote::quote!(#target_expr).to_string();
                        self.violations.push(
                            RuleViolation::new(
                                "dynamic_target",
                                Severity::Error,
                                format!(
                                    "{}: Dynamic contract target `{}` invoked without a visible allowlist check",
                                    FINDING_CODE, var_ref
                                ),
                                format!("{}:{}", self.fn_name, line),
                            )
                            .with_suggestion(
                                "Validate the target Address against an allowlist before invoking".to_string()
                            ),
                        );
                    }
                }
            }
        }

        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        // Client instantiation: Client::new(&env, &target) or ContractClient::new(&env, &target)
        if let Expr::Path(p) = &*node.func {
            if is_client_new_call(&p.path) {
                if let Some(target_expr) = node.args.iter().nth(1) {
                    if let Some(roots) = self.check_expr_for_untrusted(target_expr) {
                        let unvalidated: Vec<_> = roots
                            .iter()
                            .filter(|r| !self.validated_roots.contains(*r))
                            .cloned()
                            .collect();
                        if !unvalidated.is_empty() {
                            let line = node.span().start().line;
                            let var_ref = quote::quote!(#target_expr).to_string();
                            self.violations.push(
                                RuleViolation::new(
                                    "dynamic_target",
                                    Severity::Error,
                                    format!(
                                        "{}: Dynamic contract target `{}` instantiated without a visible allowlist check",
                                        FINDING_CODE, var_ref
                                    ),
                                    format!("{}:{}", self.fn_name, line),
                                )
                                .with_suggestion(
                                    "Validate the target Address against an allowlist before creating client".to_string()
                                ),
                            );
                        }
                    }
                }
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        if let Some(segment) = node.path.segments.last() {
            let macro_name = segment.ident.to_string();
            if matches!(
                macro_name.as_str(),
                "assert" | "assert_eq" | "assert_ne" | "require" | "ensure" | "panic"
            ) {
                let tokens_str = node.tokens.to_string();
                for var in &self.untrusted_vars {
                    if tokens_str.contains(var) {
                        if let Some(roots) = self.alias_to_root.get(var) {
                            self.validated_roots.extend(roots.clone());
                        }
                    }
                }
            }
        }
        visit::visit_macro(self, node);
    }
}

struct VarCollector<'a> {
    untrusted_vars: &'a HashSet<String>,
    found_vars: Vec<String>,
}

impl<'ast> Visit<'ast> for VarCollector<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if let Some(ident) = node.path.get_ident() {
            let name = ident.to_string();
            if self.untrusted_vars.contains(&name) {
                self.found_vars.push(name);
            }
        }
        visit::visit_expr_path(self, node);
    }
}

fn is_address_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Address"),
        Type::Reference(reference) => is_address_type(&reference.elem),
        _ => false,
    }
}

fn is_client_new_call(path: &syn::Path) -> bool {
    if let Some(last_seg) = path.segments.last() {
        if last_seg.ident == "new" {
            if path.segments.len() >= 2 {
                let prev_seg = &path.segments[path.segments.len() - 2];
                let prev_name = prev_seg.ident.to_string();
                return prev_name.ends_with("Client");
            }
        }
    }
    false
}
