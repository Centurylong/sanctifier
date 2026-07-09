use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_SHIFT_OVERFLOW";

/// Detects shifts whose runtime amount is not visibly bounded below the left
/// operand's integer width.
pub struct ShiftOverflowRule;

impl ShiftOverflowRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShiftOverflowRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ShiftOverflowRule {
    fn name(&self) -> &str {
        "shift_overflow"
    }

    fn description(&self) -> &str {
        "Detects bit shifts with unbounded or out-of-range shift amounts"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = ShiftOverflowRuleVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ShiftOverflowRuleVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for ShiftOverflowRuleVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if matches!(node.vis, syn::Visibility::Public(_)) {
            let mut function = FunctionShiftVisitor {
                function_name: node.sig.ident.to_string(),
                int_types: collect_signature_int_types(&node.sig),
                bounded: HashSet::new(),
                violations: Vec::new(),
            };
            function.visit_ordered_block(&node.block);
            self.violations.extend(function.violations);
        }
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if matches!(node.vis, syn::Visibility::Public(_)) {
            let mut function = FunctionShiftVisitor {
                function_name: node.sig.ident.to_string(),
                int_types: collect_signature_int_types(&node.sig),
                bounded: HashSet::new(),
                violations: Vec::new(),
            };
            function.visit_ordered_block(&node.block);
            self.violations.extend(function.violations);
        }
        syn::visit::visit_item_fn(self, node);
    }
}

struct FunctionShiftVisitor {
    function_name: String,
    int_types: HashMap<String, u16>,
    bounded: HashSet<String>,
    violations: Vec<RuleViolation>,
}

impl FunctionShiftVisitor {
    fn visit_ordered_block(&mut self, block: &syn::Block) {
        for stmt in &block.stmts {
            self.record_stmt_bindings(stmt);
            self.record_stmt_bounds(stmt);
            self.check_stmt(stmt);
        }
    }

    fn record_stmt_bindings(&mut self, stmt: &syn::Stmt) {
        if let syn::Stmt::Local(local) = stmt {
            if let Some((name, bits)) = typed_local_int(&local.pat) {
                self.int_types.insert(name, bits);
            }
        }
    }

    fn record_stmt_bounds(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Expr(expr, _) => self.record_expr_bounds(expr),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.record_expr_bounds(&init.expr);
                }
            }
            syn::Stmt::Macro(macro_stmt) => {
                self.record_guard_tokens(&macro_stmt.mac.tokens.to_string());
            }
            syn::Stmt::Item(_) => {}
        }
    }

    fn record_expr_bounds(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Binary(binary) => {
                if let Some(name) = bounded_shift_ident(binary) {
                    self.bounded.insert(name);
                }
                self.record_expr_bounds(&binary.left);
                self.record_expr_bounds(&binary.right);
            }
            syn::Expr::If(if_expr) => self.record_expr_bounds(&if_expr.cond),
            syn::Expr::Paren(paren) => self.record_expr_bounds(&paren.expr),
            syn::Expr::Group(group) => self.record_expr_bounds(&group.expr),
            syn::Expr::Block(block) => {
                for stmt in &block.block.stmts {
                    self.record_stmt_bounds(stmt);
                }
            }
            syn::Expr::Call(call) => {
                for arg in &call.args {
                    self.record_expr_bounds(arg);
                }
            }
            syn::Expr::MethodCall(method_call) => {
                self.record_expr_bounds(&method_call.receiver);
                for arg in &method_call.args {
                    self.record_expr_bounds(arg);
                }
            }
            _ => {}
        }
    }

    fn record_guard_tokens(&mut self, tokens: &str) {
        let compact: String = tokens.chars().filter(|ch| !ch.is_whitespace()).collect();
        for ident in identifiers_in_tokens(tokens) {
            if compact.contains(&format!("{ident}<"))
                || compact.contains(&format!("{ident}<="))
                || compact.contains(&format!(">{ident}"))
                || compact.contains(&format!(">={ident}"))
            {
                self.bounded.insert(ident);
            }
        }
    }

    fn check_stmt(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Expr(expr, _) => self.check_expr(expr),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.check_expr(&init.expr);
                }
            }
            syn::Stmt::Macro(_) | syn::Stmt::Item(_) => {}
        }
    }

    fn check_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Binary(binary) => {
                self.check_expr(&binary.left);
                self.check_expr(&binary.right);

                if let Some(operator) = shift_operator(&binary.op) {
                    let left_width = expr_int_width(&binary.left, &self.int_types);
                    if shift_rhs_is_safe(&binary.right, left_width, &self.bounded) {
                        return;
                    }

                    let detail = match left_width {
                        Some(bits) => format!(" below {bits} bits"),
                        None => " below the left operand width".to_string(),
                    };
                    self.violations.push(
                        RuleViolation::new(
                            FINDING_CODE,
                            Severity::Warning,
                            format!(
                                "{FINDING_CODE}: `{operator}` uses a shift amount without a visible bound{detail}"
                            ),
                            format!("{}:{}", self.function_name, binary.op.span().start().line),
                        )
                        .with_suggestion(
                            "Check the shift amount against the left operand bit width before shifting"
                                .to_string(),
                        ),
                    );
                }
            }
            syn::Expr::Block(block) => self.visit_ordered_block(&block.block),
            syn::Expr::If(if_expr) => {
                self.record_expr_bounds(&if_expr.cond);
                self.check_expr(&if_expr.cond);
                self.check_branch(&if_expr.then_branch);
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.check_expr(else_expr);
                }
            }
            syn::Expr::Match(match_expr) => {
                self.check_expr(&match_expr.expr);
                for arm in &match_expr.arms {
                    self.check_expr(&arm.body);
                }
            }
            syn::Expr::Paren(paren) => self.check_expr(&paren.expr),
            syn::Expr::Group(group) => self.check_expr(&group.expr),
            syn::Expr::Tuple(tuple) => {
                for elem in &tuple.elems {
                    self.check_expr(elem);
                }
            }
            syn::Expr::Array(array) => {
                for elem in &array.elems {
                    self.check_expr(elem);
                }
            }
            syn::Expr::Call(call) => {
                for arg in &call.args {
                    self.check_expr(arg);
                }
            }
            syn::Expr::MethodCall(method_call) => {
                self.check_expr(&method_call.receiver);
                for arg in &method_call.args {
                    self.check_expr(arg);
                }
            }
            syn::Expr::Assign(assign) => {
                self.check_expr(&assign.left);
                self.check_expr(&assign.right);
            }
            syn::Expr::Return(return_expr) => {
                if let Some(expr) = &return_expr.expr {
                    self.check_expr(expr);
                }
            }
            _ => {}
        }
    }

    fn check_branch(&mut self, block: &syn::Block) {
        let saved_bounds = self.bounded.clone();
        self.visit_ordered_block(block);
        self.bounded = saved_bounds;
    }
}

fn shift_operator(op: &syn::BinOp) -> Option<&'static str> {
    match op {
        syn::BinOp::Shl(_) => Some("<<"),
        syn::BinOp::Shr(_) => Some(">>"),
        syn::BinOp::ShlAssign(_) => Some("<<="),
        syn::BinOp::ShrAssign(_) => Some(">>="),
        _ => None,
    }
}

fn shift_rhs_is_safe(rhs: &syn::Expr, left_width: Option<u16>, bounded: &HashSet<String>) -> bool {
    if let Some(value) = literal_u128(rhs) {
        return left_width.is_some_and(|bits| value < bits as u128);
    }

    shift_ident(rhs).is_some_and(|ident| bounded.contains(&ident))
}

fn bounded_shift_ident(binary: &syn::ExprBinary) -> Option<String> {
    match &binary.op {
        syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Gt(_) | syn::BinOp::Ge(_) => {
            if bound_expr_mentions_width(&binary.right) || literal_u128(&binary.right).is_some() {
                shift_ident(&binary.left)
            } else if bound_expr_mentions_width(&binary.left)
                || literal_u128(&binary.left).is_some()
            {
                shift_ident(&binary.right)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn bound_expr_mentions_width(expr: &syn::Expr) -> bool {
    quote::quote!(#expr).to_string().contains("BITS")
}

fn shift_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => path
            .path
            .segments
            .first()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Paren(paren) => shift_ident(&paren.expr),
        syn::Expr::Group(group) => shift_ident(&group.expr),
        syn::Expr::Cast(cast) => shift_ident(&cast.expr),
        _ => None,
    }
}

fn expr_int_width(expr: &syn::Expr, int_types: &HashMap<String, u16>) -> Option<u16> {
    match expr {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => path
            .path
            .segments
            .first()
            .and_then(|segment| int_types.get(&segment.ident.to_string()).copied()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => int_width_from_name(lit.suffix()),
        syn::Expr::Paren(paren) => expr_int_width(&paren.expr, int_types),
        syn::Expr::Group(group) => expr_int_width(&group.expr, int_types),
        syn::Expr::Cast(cast) => type_int_width(&cast.ty),
        _ => None,
    }
}

fn collect_signature_int_types(sig: &syn::Signature) -> HashMap<String, u16> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_ty) => pat_ident(&pat_ty.pat).zip(type_int_width(&pat_ty.ty)),
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

fn typed_local_int(pat: &syn::Pat) -> Option<(String, u16)> {
    match pat {
        syn::Pat::Type(pat_ty) => pat_ident(&pat_ty.pat).zip(type_int_width(&pat_ty.ty)),
        _ => None,
    }
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

fn type_int_width(ty: &syn::Type) -> Option<u16> {
    match ty {
        syn::Type::Path(type_path) if type_path.path.segments.len() == 1 => type_path
            .path
            .segments
            .first()
            .and_then(|segment| int_width_from_name(&segment.ident.to_string())),
        syn::Type::Reference(reference) => type_int_width(&reference.elem),
        _ => None,
    }
}

fn int_width_from_name(name: &str) -> Option<u16> {
    match name {
        "u8" | "i8" => Some(8),
        "u16" | "i16" => Some(16),
        "u32" | "i32" => Some(32),
        "u64" | "i64" => Some(64),
        "u128" | "i128" => Some(128),
        "usize" | "isize" => Some(usize::BITS as u16),
        _ => None,
    }
}

fn literal_u128(expr: &syn::Expr) -> Option<u128> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => lit.base10_parse::<u128>().ok(),
        syn::Expr::Paren(paren) => literal_u128(&paren.expr),
        syn::Expr::Group(group) => literal_u128(&group.expr),
        _ => None,
    }
}

fn identifiers_in_tokens(tokens: &str) -> Vec<String> {
    let mut idents = Vec::new();
    let mut current = String::new();

    for ch in tokens.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            if current
                .chars()
                .next()
                .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
            {
                idents.push(current.clone());
            }
            current.clear();
        }
    }

    if !current.is_empty()
        && current
            .chars()
            .next()
            .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
    {
        idents.push(current);
    }

    idents
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unbounded_runtime_shift_amounts() {
        let source = r#"
            impl Contract {
                pub fn shift(mask: u64, shift: u32) -> u64 {
                    mask << shift
                }
            }
        "#;

        let findings = ShiftOverflowRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].message.contains("64 bits"));
    }

    #[test]
    fn flags_literal_shift_that_reaches_width() {
        let source = r#"
            impl Contract {
                pub fn shift(mask: u32) -> u32 {
                    mask >> 32
                }
            }
        "#;

        let findings = ShiftOverflowRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, FINDING_CODE);
    }

    #[test]
    fn accepts_prior_bound_checks() {
        let source = r#"
            impl Contract {
                pub fn shift(mask: u64, shift: u32) -> u64 {
                    if shift >= u64::BITS {
                        panic!("bad shift");
                    }
                    assert!(shift < 64);
                    mask << shift
                }
            }
        "#;

        let findings = ShiftOverflowRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_safe_literal_shift_amounts() {
        let source = r#"
            impl Contract {
                pub fn shift(mask: u64) -> u64 {
                    mask << 8
                }
            }
        "#;

        let findings = ShiftOverflowRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
