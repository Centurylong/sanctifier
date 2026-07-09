use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_ZERO_DENOMINATOR";

/// Detects division or modulo operations whose denominator is not a non-zero
/// constant and has no visible prior zero guard.
pub struct ZeroDenominatorRule;

impl ZeroDenominatorRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ZeroDenominatorRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ZeroDenominatorRule {
    fn name(&self) -> &str {
        "zero_denominator"
    }

    fn description(&self) -> &str {
        "Detects division or modulo by values without a visible zero-denominator guard"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = ZeroDenominatorRuleVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ZeroDenominatorRuleVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for ZeroDenominatorRuleVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if matches!(node.vis, syn::Visibility::Public(_)) {
            let mut function = FunctionZeroDenominatorVisitor {
                function_name: node.sig.ident.to_string(),
                guarded: HashSet::new(),
                violations: Vec::new(),
            };
            function.visit_ordered_block(&node.block);
            self.violations.extend(function.violations);
        }
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if matches!(node.vis, syn::Visibility::Public(_)) {
            let mut function = FunctionZeroDenominatorVisitor {
                function_name: node.sig.ident.to_string(),
                guarded: HashSet::new(),
                violations: Vec::new(),
            };
            function.visit_ordered_block(&node.block);
            self.violations.extend(function.violations);
        }
        syn::visit::visit_item_fn(self, node);
    }
}

struct FunctionZeroDenominatorVisitor {
    function_name: String,
    guarded: HashSet<String>,
    violations: Vec<RuleViolation>,
}

impl FunctionZeroDenominatorVisitor {
    fn visit_ordered_block(&mut self, block: &syn::Block) {
        for stmt in &block.stmts {
            self.record_stmt_guard(stmt);
            self.check_stmt(stmt);
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
            syn::Stmt::Macro(_) => {}
            syn::Stmt::Item(_) => {}
        }
    }

    fn check_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Binary(binary) => {
                self.check_expr(&binary.left);
                self.check_expr(&binary.right);

                if let Some(operator) = denominator_operator(&binary.op) {
                    if denominator_is_safe(&binary.right, &self.guarded) {
                        return;
                    }

                    self.violations.push(
                        RuleViolation::new(
                            FINDING_CODE,
                            Severity::Warning,
                            format!(
                                "{FINDING_CODE}: `{operator}` uses a denominator without a visible non-zero guard"
                            ),
                            format!(
                                "{}:{}",
                                self.function_name,
                                binary.op.span().start().line
                            ),
                        )
                        .with_suggestion(
                            "Check the denominator with `!= 0` or `> 0` before dividing or taking modulo"
                                .to_string(),
                        ),
                    );
                }
            }
            syn::Expr::Block(block) => self.visit_ordered_block(&block.block),
            syn::Expr::If(if_expr) => {
                self.record_expr_guard(&if_expr.cond);
                self.check_expr(&if_expr.cond);
                self.check_ordered_branch(&if_expr.then_branch);
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
            syn::Expr::Cast(cast) => self.check_expr(&cast.expr),
            _ => {}
        }
    }

    fn check_ordered_branch(&mut self, block: &syn::Block) {
        let saved = self.guarded.clone();
        self.visit_ordered_block(block);
        self.guarded = saved;
    }

    fn record_stmt_guard(&mut self, stmt: &syn::Stmt) {
        match stmt {
            syn::Stmt::Expr(expr, _) => self.record_expr_guard(expr),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.record_expr_guard(&init.expr);
                }
            }
            syn::Stmt::Macro(macro_stmt) => {
                let tokens = macro_stmt.mac.tokens.to_string();
                self.record_guard_tokens(&tokens);
            }
            syn::Stmt::Item(_) => {}
        }
    }

    fn record_expr_guard(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Binary(binary) => {
                if let Some(ident) = guarded_nonzero_ident(binary) {
                    self.guarded.insert(ident);
                }
                self.record_expr_guard(&binary.left);
                self.record_expr_guard(&binary.right);
            }
            syn::Expr::If(if_expr) => self.record_expr_guard(&if_expr.cond),
            syn::Expr::Paren(paren) => self.record_expr_guard(&paren.expr),
            syn::Expr::Group(group) => self.record_expr_guard(&group.expr),
            syn::Expr::Block(block) => {
                for stmt in &block.block.stmts {
                    self.record_stmt_guard(stmt);
                }
            }
            syn::Expr::Call(call) => {
                for arg in &call.args {
                    self.record_expr_guard(arg);
                }
            }
            syn::Expr::MethodCall(method_call) => {
                self.record_expr_guard(&method_call.receiver);
                for arg in &method_call.args {
                    self.record_expr_guard(arg);
                }
            }
            syn::Expr::Tuple(tuple) => {
                for elem in &tuple.elems {
                    self.record_expr_guard(elem);
                }
            }
            syn::Expr::Array(array) => {
                for elem in &array.elems {
                    self.record_expr_guard(elem);
                }
            }
            syn::Expr::Cast(cast) => self.record_expr_guard(&cast.expr),
            _ => {}
        }
    }

    fn record_guard_tokens(&mut self, tokens: &str) {
        let compact: String = tokens.chars().filter(|ch| !ch.is_whitespace()).collect();
        for ident in identifiers_in_tokens(tokens) {
            if compact.contains(&format!("{ident}!=0"))
                || compact.contains(&format!("0!={ident}"))
                || compact.contains(&format!("{ident}>0"))
                || compact.contains(&format!("0<{ident}"))
            {
                self.guarded.insert(ident);
            }
        }
    }
}

fn denominator_operator(op: &syn::BinOp) -> Option<&'static str> {
    match op {
        syn::BinOp::Div(_) => Some("/"),
        syn::BinOp::Rem(_) => Some("%"),
        _ => None,
    }
}

fn denominator_is_safe(expr: &syn::Expr, guarded: &HashSet<String>) -> bool {
    if literal_is_nonzero(expr) {
        return true;
    }

    denominator_ident(expr).is_some_and(|ident| guarded.contains(&ident))
}

fn literal_is_nonzero(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => lit.base10_parse::<i128>().is_ok_and(|value| value != 0),
        syn::Expr::Paren(paren) => literal_is_nonzero(&paren.expr),
        syn::Expr::Group(group) => literal_is_nonzero(&group.expr),
        _ => false,
    }
}

fn denominator_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => path
            .path
            .segments
            .first()
            .map(|segment| segment.ident.to_string()),
        syn::Expr::Paren(paren) => denominator_ident(&paren.expr),
        syn::Expr::Group(group) => denominator_ident(&group.expr),
        syn::Expr::Cast(cast) => denominator_ident(&cast.expr),
        _ => None,
    }
}

fn guarded_nonzero_ident(binary: &syn::ExprBinary) -> Option<String> {
    match &binary.op {
        syn::BinOp::Eq(_) => {
            if literal_is_zero(&binary.right) {
                denominator_ident(&binary.left)
            } else if literal_is_zero(&binary.left) {
                denominator_ident(&binary.right)
            } else {
                None
            }
        }
        syn::BinOp::Ne(_) => {
            if literal_is_zero(&binary.right) {
                denominator_ident(&binary.left)
            } else if literal_is_zero(&binary.left) {
                denominator_ident(&binary.right)
            } else {
                None
            }
        }
        syn::BinOp::Gt(_) => {
            if literal_is_zero(&binary.right) {
                denominator_ident(&binary.left)
            } else {
                None
            }
        }
        syn::BinOp::Lt(_) => {
            if literal_is_zero(&binary.left) {
                denominator_ident(&binary.right)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn literal_is_zero(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => lit.base10_parse::<i128>().is_ok_and(|value| value == 0),
        syn::Expr::Paren(paren) => literal_is_zero(&paren.expr),
        syn::Expr::Group(group) => literal_is_zero(&group.expr),
        _ => false,
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
    fn flags_variable_division_and_modulo_without_guard() {
        let source = r#"
            impl Contract {
                pub fn ratios(total: i128, supply: i128, period: u64) -> (i128, u64) {
                    (total / supply, period % supply as u64)
                }
            }
        "#;

        let findings = ZeroDenominatorRule::new().check(source);

        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|finding| finding.rule_name == FINDING_CODE));
    }

    #[test]
    fn accepts_prior_if_and_assert_zero_guards() {
        let source = r#"
            impl Contract {
                pub fn guarded_ratio(total: i128, supply: i128, period: u64) -> i128 {
                    if supply == 0 {
                        panic!("zero supply");
                    }
                    assert!(period > 0);
                    total / supply + total / period as i128
                }
            }
        "#;

        let findings = ZeroDenominatorRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_nonzero_literal_denominators() {
        let source = r#"
            impl Contract {
                pub fn half(total: i128) -> i128 {
                    total / 2
                }
            }
        "#;

        let findings = ZeroDenominatorRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
