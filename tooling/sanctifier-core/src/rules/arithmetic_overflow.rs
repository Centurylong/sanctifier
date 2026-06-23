use crate::rules::{Rule, RuleViolation, Severity};
use crate::ArithmeticIssue;
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

pub struct ArithmeticOverflowRule;

impl ArithmeticOverflowRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ArithmeticOverflowRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ArithmeticOverflowRule {
    fn name(&self) -> &str {
        "arithmetic_overflow"
    }

    fn description(&self) -> &str {
        "Detects unchecked arithmetic operations that could overflow or underflow"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = ArithVisitor {
            issues: Vec::new(),
            current_fn: None,
            seen: HashSet::new(),
            var_types: HashMap::new(),
        };
        visitor.visit_file(&file);

        visitor
            .issues
            .into_iter()
            .map(|issue| {
                if issue.operation.starts_with("as ") {
                    RuleViolation::new(
                        "SANCT_UNSAFE_CAST",
                        Severity::Error,
                        format!("Unsafe numeric cast detected: {}", issue.operation),
                        issue.location,
                    )
                    .with_suggestion(issue.suggestion)
                } else {
                    RuleViolation::new(
                        self.name(),
                        Severity::Warning,
                        format!("Unchecked '{}' operation could overflow", issue.operation),
                        issue.location,
                    )
                    .with_suggestion(issue.suggestion)
                }
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(crate) struct ArithVisitor {
    pub(crate) issues: Vec<ArithmeticIssue>,
    pub(crate) current_fn: Option<String>,
    pub(crate) seen: HashSet<(String, String)>,
    pub(crate) var_types: HashMap<String, IntType>,
}

// Redundant ArithmeticIssue struct removed

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IntType {
    signed: bool,
    bits: u16,
}

impl ArithVisitor {
    fn classify_op(op: &syn::BinOp) -> Option<(&'static str, &'static str)> {
        match op {
            syn::BinOp::Add(_) => Some((
                "+",
                "Use .checked_add(rhs) or .saturating_add(rhs) to handle overflow",
            )),
            syn::BinOp::Sub(_) => Some((
                "-",
                "Use .checked_sub(rhs) or .saturating_sub(rhs) to handle underflow",
            )),
            syn::BinOp::Mul(_) => Some((
                "*",
                "Use .checked_mul(rhs) or .saturating_mul(rhs) to handle overflow",
            )),
            syn::BinOp::AddAssign(_) => Some((
                "+=",
                "Replace a += b with a = a.checked_add(b).expect(\"overflow\")",
            )),
            syn::BinOp::SubAssign(_) => Some((
                "-=",
                "Replace a -= b with a = a.checked_sub(b).expect(\"underflow\")",
            )),
            syn::BinOp::MulAssign(_) => Some((
                "*=",
                "Replace a *= b with a = a.checked_mul(b).expect(\"overflow\")",
            )),
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for ArithVisitor {
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

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if let Some(fn_name) = self.current_fn.clone() {
            if let Some((op_str, suggestion)) = Self::classify_op(&node.op) {
                if !is_string_literal(&node.left) && !is_string_literal(&node.right) {
                    let key = (fn_name.clone(), op_str.to_string());
                    if !self.seen.contains(&key) {
                        self.seen.insert(key);
                        let line = node.left.span().start().line;
                        self.issues.push(ArithmeticIssue {
                            function_name: fn_name.clone(),
                            operation: op_str.to_string(),
                            suggestion: suggestion.to_string(),
                            location: format!("{}:{}", fn_name, line),
                        });
                    }
                }
            }
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(fn_name) = self.current_fn.clone() {
            let method_name = node.method.to_string();
            if let Some(suggestion) = classify_math_method(&method_name) {
                let key = (fn_name.clone(), method_name.clone());
                if !self.seen.contains(&key) {
                    self.seen.insert(key);
                    let line = node.span().start().line;
                    self.issues.push(ArithmeticIssue {
                        function_name: fn_name.clone(),
                        operation: method_name,
                        suggestion,
                        location: format!("{}:{}", fn_name, line),
                    });
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Some(fn_name) = self.current_fn.clone() {
            if let syn::Expr::Path(expr_path) = &*node.func {
                if let Some(last_segment) = expr_path.path.segments.last() {
                    let func_name = last_segment.ident.to_string();
                    if let Some(suggestion) = classify_math_call(&func_name) {
                        let key = (fn_name.clone(), func_name.clone());
                        if !self.seen.contains(&key) {
                            self.seen.insert(key);
                            let line = node.span().start().line;
                            self.issues.push(ArithmeticIssue {
                                function_name: fn_name.clone(),
                                operation: func_name,
                                suggestion,
                                location: format!("{}:{}", fn_name, line),
                            });
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_cast(&mut self, node: &'ast syn::ExprCast) {
        if let Some(fn_name) = self.current_fn.clone() {
            if let (Some(source), Some(target)) = (
                int_type_from_expr(&node.expr, &self.var_types),
                int_type_from_type(&node.ty),
            ) {
                if is_lossy_cast(source, target) {
                    let operation = format!("as {}", int_type_label(target));
                    let key = (
                        fn_name.clone(),
                        format!("{}:{}", operation, node.span().start().line),
                    );
                    if !self.seen.contains(&key) {
                        self.seen.insert(key);
                        let line = node.span().start().line;
                        self.issues.push(ArithmeticIssue {
                            function_name: fn_name.clone(),
                            operation: format!(
                                "as {} from {}",
                                int_type_label(target),
                                int_type_label(source)
                            ),
                            suggestion: "Use TryInto/try_into and handle the conversion error instead of a lossy `as` cast".to_string(),
                            location: format!("{}:{}", fn_name, line),
                        });
                    }
                }
            }
        }
        syn::visit::visit_expr_cast(self, node);
    }
}

fn classify_math_method(method: &str) -> Option<String> {
    match method {
        "mul_div" => Some("Use '.checked_mul_div()' to handle potential overflow".to_string()),
        "div_ceil" => {
            Some("Consider '.checked_div()' if boundary verification is required".to_string())
        }
        "fixed_point_mul" => Some("Use '.checked_fixed_point_mul()' for safety".to_string()),
        "fixed_point_div" => Some("Use '.checked_fixed_point_div()' for safety".to_string()),
        _ => None,
    }
}

fn classify_math_call(func: &str) -> Option<String> {
    match func {
        "mul_div" => Some("Use 'checked_mul_div' to handle potential overflow".to_string()),
        "fixed_point_mul" => Some("Use 'checked_fixed_point_mul' for safety".to_string()),
        "fixed_point_div" => Some("Use 'checked_fixed_point_div' for safety".to_string()),
        _ => None,
    }
}

fn is_string_literal(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(_),
            ..
        })
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag_standard_arithmetic() {
        let rule = ArithmeticOverflowRule::new();
        let source = r#"
            fn test() {
                let a = 1;
                let b = 2;
                let c = a + b;
                let d = a - b;
                let e = a * b;
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 3);
    }

    #[test]
    fn test_flag_custom_math_methods() {
        let rule = ArithmeticOverflowRule::new();
        let source = r#"
            fn test() {
                let a = 1;
                let b = 2;
                let c = a.mul_div(5, 10);
                let d = a.fixed_point_mul(b);
            }
        "#;
        let violations = rule.check(source);
        assert!(violations.iter().any(|v| v.message.contains("mul_div")));
        assert!(violations
            .iter()
            .any(|v| v.message.contains("fixed_point_mul")));
    }

    #[test]
    fn test_flag_custom_math_calls() {
        let rule = ArithmeticOverflowRule::new();
        let source = r#"
            fn test() {
                let a = mul_div(1, 2, 3);
                let b = fixed_point_div(10, 2);
            }
        "#;
        let violations = rule.check(source);
        assert!(violations.iter().any(|v| v.message.contains("mul_div")));
        assert!(violations
            .iter()
            .any(|v| v.message.contains("fixed_point_div")));
    }

    #[test]
    fn test_ignore_checked_methods() {
        let rule = ArithmeticOverflowRule::new();
        let source = r#"
            fn test() {
                let a = 1;
                let b = a.checked_add(2);
                let c = a.checked_mul_div(5, 10);
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_flag_lossy_integer_casts() {
        let rule = ArithmeticOverflowRule::new();
        let source = r#"
            fn convert(big: i128, balance: u64, signed: i64) {
                let amount_u32 = big as u32;
                let index_i32 = balance as i32;
                let wrapped_u64 = signed as u64;
            }
        "#;

        let violations = rule.check(source);

        assert_eq!(violations.len(), 3);
        assert!(violations
            .iter()
            .all(|v| v.rule_name == "SANCT_UNSAFE_CAST"));
        assert!(violations.iter().all(|v| v.severity == Severity::Error));
        assert!(violations
            .iter()
            .all(|v| v.message.contains("Unsafe numeric cast")));
        assert!(violations.iter().all(|v| v
            .suggestion
            .as_deref()
            .unwrap_or_default()
            .contains("try_into")));
    }

    #[test]
    fn test_ignore_widening_and_checked_conversions() {
        let rule = ArithmeticOverflowRule::new();
        let source = r#"
            fn convert(small: u32, signed: i32) -> Result<u64, ()> {
                let widened_u64 = small as u64;
                let widened_i128 = signed as i128;
                let checked_u32: u32 = signed.try_into().map_err(|_| ())?;
                Ok(widened_u64 + checked_u32 as u64)
            }
        "#;

        let violations = rule.check(source);

        assert!(violations
            .iter()
            .all(|v| v.rule_name != "SANCT_UNSAFE_CAST"));
    }
}
