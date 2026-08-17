use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects `end - start` (or `end_ledger - start_ledger`, `end_time -
/// start_time`, ...) duration/schedule arithmetic that is not proven
/// `end > start` first. Linear-vesting math divides the vested amount by this
/// span; when `end <= start` it either panics (unsigned underflow /
/// division-by-zero further downstream) or silently yields a nonsense
/// schedule, releasing funds all at once or never.
pub struct VestingScheduleRule;

impl VestingScheduleRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VestingScheduleRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for VestingScheduleRule {
    fn name(&self) -> &str {
        "vesting_schedule"
    }

    fn description(&self) -> &str {
        "Detects vesting/schedule duration arithmetic (`end - start`) that lacks a prior \
         `end > start` validation guard"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = VestingVisitor {
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

struct VestingVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
    /// (start, end) identifier pairs proven `end > start` in the current scope.
    guarded: HashSet<(String, String)>,
    /// (function, start, end) triples already reported, to avoid duplicates.
    seen: HashSet<(String, String, String)>,
}

impl VestingVisitor {
    /// Walk a block's statements in order, tracking range guards introduced by
    /// preceding sibling statements (early-return/panic/assert style) and by
    /// `if end > start { .. }` wrapping (scoped to that branch only).
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
                syn::Stmt::Macro(m) => {
                    if let Some(pair) = assert_guard_target(&m.mac) {
                        self.guarded.insert(pair);
                    }
                }
                syn::Stmt::Item(_) => {}
            }

            // An `if end <= start { <diverges> }` guard (in either variable
            // order) as a sibling statement protects every statement *after*
            // it in this same block.
            if let syn::Stmt::Expr(syn::Expr::If(if_expr), _) = stmt {
                if let Some(pair) = bad_range_check(&if_expr.cond) {
                    if branch_diverges(&if_expr.then_branch) {
                        self.guarded.insert(pair);
                    }
                }
            }
        }
    }

    fn walk_if(&mut self, if_expr: &syn::ExprIf) {
        self.walk_expr(&if_expr.cond);

        if let Some(pair) = good_range_check(&if_expr.cond) {
            // `if end > start { .. }`: the then-branch alone is guarded.
            let inserted = self.guarded.insert(pair.clone());
            self.walk_block(&if_expr.then_branch);
            if inserted {
                self.guarded.remove(&pair);
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
            if matches!(bin.op, syn::BinOp::Sub(_)) {
                self.check_span(&bin.left, &bin.right, bin.span());
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

    /// `left - right`: only interesting when `left` looks like an "end"
    /// bound and `right` looks like a "start" bound (by identifier name).
    fn check_span(&mut self, left: &syn::Expr, right: &syn::Expr, span: proc_macro2::Span) {
        let Some(end_name) = simple_ident(left) else {
            return;
        };
        let Some(start_name) = simple_ident(right) else {
            return;
        };
        if !looks_like_end(&end_name) || !looks_like_start(&start_name) {
            return;
        }

        let pair = (start_name.clone(), end_name.clone());
        if self.guarded.contains(&pair) {
            return;
        }
        self.report(start_name, end_name, span);
    }

    fn report(&mut self, start_name: String, end_name: String, span: proc_macro2::Span) {
        let fn_name = self
            .current_fn
            .clone()
            .unwrap_or_else(|| "<unknown>".to_string());
        let key = (fn_name.clone(), start_name.clone(), end_name.clone());
        if !self.seen.insert(key) {
            return;
        }

        let line = span.start().line;
        self.violations.push(
            RuleViolation::new(
                "vesting_schedule",
                Severity::Warning,
                format!(
                    "`{end_name} - {start_name}` computes a schedule duration without first \
                     proving `{end_name} > {start_name}`; a caller-supplied `{end_name} <= \
                     {start_name}` panics (unsigned underflow) or silently produces a \
                     zero/negative-length schedule"
                ),
                format!("{fn_name}:{line}"),
            )
            .with_suggestion(format!(
                "Validate the range before using it, e.g. `if {end_name} <= {start_name} {{ \
                 return Err(...); }}` or `assert!({end_name} > {start_name})`"
            )),
        );
    }
}

impl<'ast> Visit<'ast> for VestingVisitor {
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

/// Returns the identifier name if `expr` is a bare variable reference (`x`,
/// possibly through a single-segment path), otherwise `None`.
fn simple_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) if p.path.segments.len() == 1 => {
            Some(p.path.segments[0].ident.to_string())
        }
        syn::Expr::Paren(p) => simple_ident(&p.expr),
        syn::Expr::Field(f) => {
            if let syn::Member::Named(ident) = &f.member {
                Some(ident.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn looks_like_end(name: &str) -> bool {
    name.to_ascii_lowercase().contains("end")
}

fn looks_like_start(name: &str) -> bool {
    name.to_ascii_lowercase().contains("start")
}

/// If `cond` is (an `&&` chain containing) `end <= start` or `start >= end`,
/// returns `(start, end)`.
fn bad_range_check(cond: &syn::Expr) -> Option<(String, String)> {
    range_check_impl(cond, is_le_or_reverse_ge)
}

/// If `cond` is (an `&&` chain containing) `end > start` or `start < end`,
/// returns `(start, end)`.
fn good_range_check(cond: &syn::Expr) -> Option<(String, String)> {
    range_check_impl(cond, is_gt_or_reverse_lt)
}

/// Matches `end <op> start` (predicate decides which `(BinOp, order)` counts)
/// through an `&&` chain and parens, returning the bound names as `(start, end)`.
fn range_check_impl(
    expr: &syn::Expr,
    op_matches: fn(&syn::BinOp, &str, &str) -> Option<(String, String)>,
) -> Option<(String, String)> {
    if let syn::Expr::Binary(b) = expr {
        if let (Some(l), Some(r)) = (simple_ident(&b.left), simple_ident(&b.right)) {
            if let Some(pair) = op_matches(&b.op, &l, &r) {
                return Some(pair);
            }
        }
        if matches!(b.op, syn::BinOp::And(_)) {
            return range_check_impl(&b.left, op_matches)
                .or_else(|| range_check_impl(&b.right, op_matches));
        }
    }
    if let syn::Expr::Paren(p) = expr {
        return range_check_impl(&p.expr, op_matches);
    }
    None
}

/// `left <= right` where `left` is end-like and `right` is start-like, OR
/// `left >= right` where `left` is start-like and `right` is end-like.
fn is_le_or_reverse_ge(op: &syn::BinOp, left: &str, right: &str) -> Option<(String, String)> {
    match op {
        syn::BinOp::Le(_) if looks_like_end(left) && looks_like_start(right) => {
            Some((right.to_string(), left.to_string()))
        }
        syn::BinOp::Ge(_) if looks_like_start(left) && looks_like_end(right) => {
            Some((left.to_string(), right.to_string()))
        }
        _ => None,
    }
}

/// `left > right` where `left` is end-like and `right` is start-like, OR
/// `left < right` where `left` is start-like and `right` is end-like.
fn is_gt_or_reverse_lt(op: &syn::BinOp, left: &str, right: &str) -> Option<(String, String)> {
    match op {
        syn::BinOp::Gt(_) if looks_like_end(left) && looks_like_start(right) => {
            Some((right.to_string(), left.to_string()))
        }
        syn::BinOp::Lt(_) if looks_like_start(left) && looks_like_end(right) => {
            Some((left.to_string(), right.to_string()))
        }
        _ => None,
    }
}

/// `assert!(end > start, ..)` / `assert!(start < end, ..)` as a bare
/// statement guards the remainder of the block, same as an early-return `if`.
fn assert_guard_target(mac: &syn::Macro) -> Option<(String, String)> {
    let name = mac.path.segments.last()?.ident.to_string();
    if name != "assert" {
        return None;
    }
    let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let args = parser.parse2(mac.tokens.clone()).ok()?;
    let cond = args.first()?;
    good_range_check(cond)
}

/// True if the block always diverges (returns, panics, or otherwise never
/// falls through) — i.e. it's a valid early-exit guard.
fn branch_diverges(block: &syn::Block) -> bool {
    let Some(last) = block.stmts.last() else {
        return false;
    };
    match last {
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
    fn flags_unguarded_span() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32, amount: i128) -> i128 {
                    let duration = end_ledger - start_ledger;
                    amount / (duration as i128)
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].message.contains("end_ledger"));
        assert!(v[0].message.contains("start_ledger"));
    }

    #[test]
    fn recognizes_sibling_early_return_guard() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32) -> u32 {
                    if end_ledger <= start_ledger {
                        return 0;
                    }
                    end_ledger - start_ledger
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn recognizes_sibling_panic_guard_reverse_order() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn schedule(env: Env, start_time: u64, end_time: u64) -> u64 {
                    if start_time >= end_time {
                        panic!("bad schedule");
                    }
                    end_time - start_time
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn recognizes_wrapping_guard() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32) -> u32 {
                    if end_ledger > start_ledger {
                        end_ledger - start_ledger
                    } else {
                        0
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn recognizes_bare_assert_guard() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32) -> u32 {
                    assert!(end_ledger > start_ledger, "invalid schedule");
                    end_ledger - start_ledger
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn ignores_unrelated_subtraction() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn total(env: Env, a: i128, b: i128) -> i128 {
                    a - b
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty(), "{v:#?}");
    }

    #[test]
    fn guard_does_not_leak_to_unrelated_pair() {
        let rule = VestingScheduleRule::new();
        let source = r#"
            impl Contract {
                pub fn schedule(env: Env, start_ledger: u32, end_ledger: u32, other_end: u32) -> u32 {
                    if end_ledger <= start_ledger {
                        return 0;
                    }
                    other_end - start_ledger
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1, "{v:#?}");
        assert!(v[0].message.contains("other_end"));
    }
}
