use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_EVENT_DATA_CAST";

pub struct EventDataCastRule;

impl EventDataCastRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EventDataCastRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for EventDataCastRule {
    fn name(&self) -> &str {
        "event_data_cast"
    }

    fn description(&self) -> &str {
        "Detects narrowing integer casts in event emission data that silently truncate values indexers see"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = EventDataCastVisitor {
            violations: Vec::new(),
            current_fn: None,
            var_types: HashMap::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// ── Integer type helpers ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntType {
    signed: bool,
    bits: u16,
}

fn int_type_from_type(ty: &syn::Type) -> Option<IntType> {
    match ty {
        syn::Type::Path(type_path) if type_path.path.segments.len() == 1 => type_path
            .path
            .segments
            .first()
            .and_then(|segment| int_type_from_str(&segment.ident.to_string())),
        _ => None,
    }
}

fn int_type_from_expr(expr: &syn::Expr, var_types: &HashMap<String, IntType>) -> Option<IntType> {
    match expr {
        syn::Expr::Path(expr_path) if expr_path.path.segments.len() == 1 => expr_path
            .path
            .segments
            .first()
            .and_then(|segment| var_types.get(&segment.ident.to_string()).copied()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => int_type_from_str(lit.suffix()),
        syn::Expr::Paren(paren) => int_type_from_expr(&paren.expr, var_types),
        syn::Expr::Group(group) => int_type_from_expr(&group.expr, var_types),
        _ => None,
    }
}

fn int_type_from_str(name: &str) -> Option<IntType> {
    let (signed, bits) = match name {
        "i8" => (true, 8),
        "i16" => (true, 16),
        "i32" => (true, 32),
        "i64" => (true, 64),
        "i128" => (true, 128),
        "isize" => (true, usize::BITS as u16),
        "u8" => (false, 8),
        "u16" => (false, 16),
        "u32" => (false, 32),
        "u64" => (false, 64),
        "u128" => (false, 128),
        "usize" => (false, usize::BITS as u16),
        _ => return None,
    };
    Some(IntType { signed, bits })
}

fn is_lossy_cast(source: IntType, target: IntType) -> bool {
    target.bits < source.bits || target.signed != source.signed
}

fn int_type_label(ty: IntType) -> String {
    let prefix = if ty.signed { "i" } else { "u" };
    format!("{prefix}{}", ty.bits)
}

fn collect_signature_int_types(sig: &syn::Signature) -> HashMap<String, IntType> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_ty) => pat_ident(&pat_ty.pat).zip(int_type_from_type(&pat_ty.ty)),
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

fn local_int_binding(pat: &syn::Pat) -> Option<(String, IntType)> {
    match pat {
        syn::Pat::Type(pat_ty) => pat_ident(&pat_ty.pat).zip(int_type_from_type(&pat_ty.ty)),
        _ => None,
    }
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

// ── Visitor ──────────────────────────────────────────────────────────

struct EventDataCastVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
    var_types: HashMap<String, IntType>,
}

impl<'ast> Visit<'ast> for EventDataCastVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev = self.current_fn.take();
        let prev_types = std::mem::take(&mut self.var_types);
        self.current_fn = Some(node.sig.ident.to_string());
        self.var_types = collect_signature_int_types(&node.sig);
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = prev;
        self.var_types = prev_types;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev = self.current_fn.take();
        let prev_types = std::mem::take(&mut self.var_types);
        self.current_fn = Some(node.sig.ident.to_string());
        self.var_types = collect_signature_int_types(&node.sig);
        syn::visit::visit_item_fn(self, node);
        self.current_fn = prev;
        self.var_types = prev_types;
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some((ident, int_type)) = local_int_binding(&node.pat) {
            self.var_types.insert(ident, int_type);
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "publish" {
            if let syn::Expr::MethodCall(inner) = &*node.receiver {
                if inner.method == "events" {
                    if let syn::Expr::Path(path) = &*inner.receiver {
                        if path.path.is_ident("env") {
                            if let Some(fn_name) = &self.current_fn {
                                for arg in &node.args {
                                    scan_for_lossy_casts(
                                        arg,
                                        fn_name,
                                        &self.var_types,
                                        &mut self.violations,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

// ── Recursive cast scanner ───────────────────────────────────────────

fn scan_for_lossy_casts(
    expr: &syn::Expr,
    fn_name: &str,
    var_types: &HashMap<String, IntType>,
    violations: &mut Vec<RuleViolation>,
) {
    match expr {
        syn::Expr::Cast(cast) => {
            if let (Some(source), Some(target)) = (
                int_type_from_expr(&cast.expr, var_types),
                int_type_from_type(&cast.ty),
            ) {
                if is_lossy_cast(source, target) {
                    let line = cast.span().start().line;
                    violations.push(
                        RuleViolation::new(
                            FINDING_CODE,
                            Severity::Warning,
                            format!(
                                "{}: narrowing cast `{} as {}` in event data — \
                                 indexers receive truncated value",
                                FINDING_CODE,
                                int_type_label(source),
                                int_type_label(target),
                            ),
                            format!("{}:{}", fn_name, line),
                        )
                        .with_suggestion(
                            "Emit the full-width value in the event to prevent \
                             data loss for indexers"
                                .to_string(),
                        ),
                    );
                }
            }
        }
        syn::Expr::Tuple(tuple) => {
            for elem in &tuple.elems {
                scan_for_lossy_casts(elem, fn_name, var_types, violations);
            }
        }
        syn::Expr::Paren(paren) => {
            scan_for_lossy_casts(&paren.expr, fn_name, var_types, violations);
        }
        syn::Expr::MethodCall(m) => {
            for arg in &m.args {
                scan_for_lossy_casts(arg, fn_name, var_types, violations);
            }
        }
        syn::Expr::Call(c) => {
            for arg in &c.args {
                scan_for_lossy_casts(arg, fn_name, var_types, violations);
            }
        }
        syn::Expr::Binary(b) => {
            scan_for_lossy_casts(&b.left, fn_name, var_types, violations);
            scan_for_lossy_casts(&b.right, fn_name, var_types, violations);
        }
        syn::Expr::Unary(u) => {
            scan_for_lossy_casts(&u.expr, fn_name, var_types, violations);
        }
        _ => {}
    }
}

// ── Unit tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flags_narrowing_cast_in_event_data() {
        let rule = EventDataCastRule::new();
        let source = r#"
            use soroban_sdk::{contractimpl, symbol_short, Env};

            #[contractimpl]
            impl Contract {
                pub fn deposit(env: Env, amount: i128) {
                    env.events().publish(
                        (symbol_short!("DEPOSIT"),),
                        (amount as u32,),
                    );
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_name, FINDING_CODE);
        assert!(violations[0].message.contains("narrowing cast"));
        assert!(violations[0].location.contains("deposit"));
    }

    #[test]
    fn test_ignores_widening_cast() {
        let rule = EventDataCastRule::new();
        let source = r#"
            use soroban_sdk::{contractimpl, symbol_short, Env};

            #[contractimpl]
            impl Contract {
                pub fn deposit(env: Env, amount: u32) {
                    env.events().publish(
                        (symbol_short!("DEPOSIT"),),
                        (amount as u64,),
                    );
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_ignores_cast_outside_event_context() {
        let rule = EventDataCastRule::new();
        let source = r#"
            impl Contract {
                pub fn truncate(_env: (), amount: i128) -> u32 {
                    amount as u32
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_flags_multiple_casts_in_event() {
        let rule = EventDataCastRule::new();
        let source = r#"
            use soroban_sdk::{contractimpl, symbol_short, Env};

            #[contractimpl]
            impl Contract {
                pub fn swap(env: Env, amount_in: i128, amount_out: i64) {
                    env.events().publish(
                        (symbol_short!("SWAP"),),
                        (amount_in as u64, amount_out as u32),
                    );
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_flags_signedness_change() {
        let rule = EventDataCastRule::new();
        let source = r#"
            use soroban_sdk::{contractimpl, symbol_short, Env};

            #[contractimpl]
            impl Contract {
                pub fn wrap(env: Env, amount: i64) {
                    env.events().publish(
                        (symbol_short!("WRAP"),),
                        (amount as u64,),
                    );
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
    }

    #[test]
    fn test_ignores_no_cast_event() {
        let rule = EventDataCastRule::new();
        let source = r#"
            use soroban_sdk::{contractimpl, symbol_short, Env};

            #[contractimpl]
            impl Contract {
                pub fn deposit(env: Env, amount: i128) {
                    env.events().publish(
                        (symbol_short!("DEPOSIT"),),
                        (amount,),
                    );
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 0);
    }
}
