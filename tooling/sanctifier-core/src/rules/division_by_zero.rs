use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects `/` or `%` by a non-constant denominator that isn't proven non-zero
/// first, which panics (aborts the transaction) on Soroban's host when the
/// denominator happens to be zero at runtime.
pub struct DivisionByZeroRule;

impl DivisionByZeroRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DivisionByZeroRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for DivisionByZeroRule {
    fn name(&self) -> &str {
        "division_by_zero"
    }

    fn description(&self) -> &str {
        "Detects division or modulo by a non-constant value that could be zero at runtime, \
         without a prior zero-check guard"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = DivByZeroVisitor {
            violations: Vec::new(),
            current_fn: None,
            guarded: HashSet::new(),
            seen: HashSet::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct DivByZeroVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
    /// Denominator identifiers proven non-zero in the current lexical scope.
    guarded: HashSet<String>,
    /// (function, denominator) pairs already reported, to avoid duplicate findings.
    seen: HashSet<(String, String)>,
}

impl DivByZeroVisitor {
    /// Walk a block's statements in order, tracking zero-guards introduced by
    /// preceding sibling statements (early-return style) and by `if x != 0 { .. }`
    /// wrapping (scoped to that branch only).
    fn walk_block(&mut self, block: &syn::Block) {
        for stmt in &block.stmts {
            match stmt {
                syn::Stmt::Expr(syn::Expr::If(if_expr), _) => {
                    self.walk_if(if_expr);
                }
                syn::Stmt::Local(local) => {
                    if let Some(init) = &local.init {
                        self.walk_expr(&init.expr);
                    }
                }
                syn::Stmt::Expr(expr, _) => self.walk_expr(expr),
                syn::Stmt::Macro(_) | syn::Stmt::Item(_) => {}
            }

            // An `if denom == 0 { <diverges> }` guard as a sibling statement
            // protects every statement *after* it in this same block.
            if let syn::Stmt::Expr(syn::Expr::If(if_expr), _) = stmt {
                if let Some(name) = zero_check_target(&if_expr.cond) {
                    if branch_diverges(&if_expr.then_branch) {
                        self.guarded.insert(name);
                    }
                }
            }
        }
    }

    fn walk_if(&mut self, if_expr: &syn::ExprIf) {
        self.walk_expr(&if_expr.cond);

        if let Some(name) = nonzero_check_target(&if_expr.cond) {
            // `if denom != 0 { .. }`: the then-branch alone is guarded.
            let inserted = self.guarded.insert(name.clone());
            self.walk_block(&if_expr.then_branch);
            if inserted {
                self.guarded.remove(&name);
            }
        } else {
            self.walk_block(&if_expr.then_branch);
        }

        if let Some((_, else_branch)) = &if_expr.else_branch {
            self.walk_expr(else_branch);
        }
    }

    fn walk_expr(&mut self, expr: &syn::Expr) {
        if let syn::Expr::Binary(bin) = expr {
            if matches!(bin.op, syn::BinOp::Div(_) | syn::BinOp::Rem(_)) {
                self.check_denominator(&bin.right, bin.span());
            }
            self.walk_expr(&bin.left);
            self.walk_expr(&bin.right);
            return;
        }

        match expr {
            syn::Expr::If(if_expr) => self.walk_if(if_expr),
            syn::Expr::Block(b) => self.walk_block(&b.block),
            syn::Expr::Paren(p) => self.walk_expr(&p.expr),
            syn::Expr::Let(l) => self.walk_expr(&l.expr),
            syn::Expr::Return(r) => {
                if let Some(e) = &r.expr {
                    self.walk_expr(e);
                }
            }
            syn::Expr::Assign(a) => {
                self.walk_expr(&a.left);
                self.walk_expr(&a.right);
            }
            _ => {}
        }
    }

    fn check_denominator(&mut self, denom: &syn::Expr, span: proc_macro2::Span) {
        // Constants (integer/float literals, possibly negated) never trigger:
        // rustc itself rejects a literal-zero denominator at compile time.
        if is_constant(denom) {
            return;
        }

        let Some(name) = simple_ident(denom) else {
            // Complex denominators (method calls, field access, etc.) can't be
            // matched against a guard reliably, so always flag them.
            self.report(denom_display(denom), span);
            return;
        };

        if self.guarded.contains(&name) {
            return;
        }
        self.report(name, span);
    }

    fn report(&mut self, denom_name: String, span: proc_macro2::Span) {
        let fn_name = self
            .current_fn
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        let key = (fn_name.clone(), denom_name.clone());
        if !self.seen.insert(key) {
            return;
        }

        let line = span.start().line;
        self.violations.push(
            RuleViolation::new(
                "division_by_zero",
                Severity::Warning,
                format!(
                    "Division/modulo by `{denom_name}`, which is not proven non-zero, \
                     panics on-chain if it is zero at runtime"
                ),
                format!("{fn_name}:{line}"),
            )
            .with_suggestion(format!(
                "Add a guard before dividing, e.g. `if {denom_name} == 0 {{ return Err(...); }}` \
                 or use `.checked_div()`/`.checked_rem()`"
            )),
        );
    }
}

impl<'ast> Visit<'ast> for DivByZeroVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev_fn = self.current_fn.replace(node.sig.ident.to_string());
        let prev_guarded = self.guarded.clone();
        self.walk_block(&node.block);
        self.current_fn = prev_fn;
        self.guarded = prev_guarded;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev_fn = self.current_fn.replace(node.sig.ident.to_string());
        let prev_guarded = self.guarded.clone();
        self.walk_block(&node.block);
        self.current_fn = prev_fn;
        self.guarded = prev_guarded;
    }
}

/// True for integer/float literals, including negated ones (`-1`), which are
/// always known at compile time.
fn is_constant(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(_) | syn::Lit::Float(_),
            ..
        }) => true,
        syn::Expr::Unary(u) => matches!(u.op, syn::UnOp::Neg(_)) && is_constant(&u.expr),
        syn::Expr::Paren(p) => is_constant(&p.expr),
        _ => false,
    }
}

/// Returns the identifier name if `expr` is a bare variable reference (`x`,
/// possibly through a single-segment path), otherwise `None`.
fn simple_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) if p.path.segments.len() == 1 => {
            Some(p.path.segments[0].ident.to_string())
        }
        syn::Expr::Paren(p) => simple_ident(&p.expr),
        _ => None,
    }
}

/// A human-readable label for denominators that aren't a simple identifier.
fn denom_display(expr: &syn::Expr) -> String {
    quote::quote!(#expr).to_string()
}

/// If `cond` is (an `&&` chain containing) `name == 0`, returns `name`.
fn zero_check_target(cond: &syn::Expr) -> Option<String> {
    binary_zero_check(cond, syn_eq_matches)
}

/// If `cond` is (an `&&` chain containing) `name != 0`, returns `name`.
fn nonzero_check_target(cond: &syn::Expr) -> Option<String> {
    binary_zero_check(cond, syn_ne_matches)
}

fn binary_zero_check(expr: &syn::Expr, op_matches: fn(&syn::BinOp) -> bool) -> Option<String> {
    if let syn::Expr::Binary(b) = expr {
        if op_matches(&b.op) {
            if let Some(name) = simple_ident(&b.left) {
                if is_zero_literal(&b.right) {
                    return Some(name);
                }
            }
            if let Some(name) = simple_ident(&b.right) {
                if is_zero_literal(&b.left) {
                    return Some(name);
                }
            }
        }
        if matches!(b.op, syn::BinOp::And(_)) {
            return binary_zero_check(&b.left, op_matches)
                .or_else(|| binary_zero_check(&b.right, op_matches));
        }
    }
    if let syn::Expr::Paren(p) = expr {
        return binary_zero_check(&p.expr, op_matches);
    }
    None
}

fn syn_eq_matches(op: &syn::BinOp) -> bool {
    matches!(op, syn::BinOp::Eq(_))
}

fn syn_ne_matches(op: &syn::BinOp) -> bool {
    matches!(op, syn::BinOp::Ne(_))
}

fn is_zero_literal(expr: &syn::Expr) -> bool {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(n),
        ..
    }) = expr
    {
        n.base10_parse::<i64>().unwrap_or(1) == 0
    } else {
        false
    }
}

/// True if the block always diverges (returns, panics, or otherwise never
/// falls through) — i.e. it's a valid early-exit guard.
fn branch_diverges(block: &syn::Block) -> bool {
    let Some(last) = block.stmts.last() else {
        return false;
    };
    match last {
        // A macro-with-semicolon statement (`panic!(..);`) parses as `Stmt::Macro`
        // in syn 2, not `Stmt::Expr(Expr::Macro, _)`.
        syn::Stmt::Macro(m) => macro_diverges(&m.mac),
        syn::Stmt::Expr(expr, _) => expr_diverges(expr),
        _ => false,
    }
}

/// True if `expr` is a diverging tail expression (early return, panic, etc.).
fn expr_diverges(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Return(_) => true,
        syn::Expr::Macro(m) => macro_diverges(&m.mac),
        syn::Expr::MethodCall(m) => matches!(m.method.to_string().as_str(), "expect" | "unwrap"),
        syn::Expr::Continue(_) => true,
        _ => false,
    }
}

/// True for macros that never return normally (`panic!`, `unreachable!`, `assert*!`).
fn macro_diverges(mac: &syn::Macro) -> bool {
    let name = mac
        .path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();
    matches!(
        name.as_str(),
        "panic" | "unreachable" | "assert" | "assert_eq" | "assert_ne"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_bare_division_by_variable() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn split(env: Env, total: i128, count: i128) -> i128 {
                    total / count
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("count"));
    }

    #[test]
    fn flags_modulo_by_variable() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn pick(env: Env, seed: u64, players: u32) -> u32 {
                    (seed as u32) % players
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("players"));
    }

    #[test]
    fn ignores_constant_denominator() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn bps(env: Env, amount: i128) -> i128 {
                    amount / 10_000
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn recognizes_sibling_early_return_guard() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn split(env: Env, total: i128, count: i128) -> i128 {
                    if count == 0 {
                        return 0;
                    }
                    total / count
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn recognizes_sibling_panic_guard() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn split(env: Env, total: i128, count: i128) -> i128 {
                    if count == 0 {
                        panic!("count must not be zero");
                    }
                    total / count
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn recognizes_wrapping_nonzero_guard() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn split(env: Env, total: i128, count: i128) -> i128 {
                    if count != 0 {
                        total / count
                    } else {
                        0
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn guard_does_not_leak_to_unrelated_variable() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn split(env: Env, total: i128, count: i128, other: i128) -> i128 {
                    if count == 0 {
                        return 0;
                    }
                    total / other
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("other"));
    }

    #[test]
    fn ignores_checked_div_and_rem() {
        let rule = DivisionByZeroRule::new();
        let source = r#"
            impl Contract {
                pub fn split(env: Env, total: i128, count: i128) -> i128 {
                    total.checked_div(count).unwrap_or(0)
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }
}
