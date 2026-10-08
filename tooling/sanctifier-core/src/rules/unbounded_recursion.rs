use crate::finding_codes::UNBOUNDED_RECURSION;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Attribute, Expr, FnArg, Pat, Signature};

/// Detects direct self-recursion that has no explicit depth bound.
///
/// This rule is deliberately intraprocedural. It flags a function only when it
/// can see that function call itself, and treats recursion as bounded only when
/// a depth-like parameter is both checked by a comparison and changed
/// monotonically at the recursive call site.
pub struct UnboundedRecursionRule;

impl UnboundedRecursionRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnboundedRecursionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UnboundedRecursionRule {
    fn name(&self) -> &str {
        "unbounded_recursion"
    }

    fn description(&self) -> &str {
        "Detects direct self-recursion without an explicit monotonic depth bound"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };

        let mut visitor = FunctionVisitor {
            violations: Vec::new(),
            test_depth: 0,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FunctionVisitor {
    violations: Vec<RuleViolation>,
    test_depth: usize,
}

impl FunctionVisitor {
    fn check_function(&mut self, sig: &Signature, block: &syn::Block) {
        if self.test_depth > 0 {
            return;
        }

        let fn_name = sig.ident.to_string();
        let depth_params = depth_parameter_names(sig);
        let param_positions = typed_parameter_positions(sig);
        let mut facts = RecursionFacts {
            fn_name: &fn_name,
            depth_params: &depth_params,
            param_positions: &param_positions,
            recursive_calls: 0,
            bounded_recursive_calls: 0,
            guarded_depth_params: HashSet::new(),
        };
        facts.visit_block(block);

        if facts.recursive_calls == 0 {
            return;
        }

        // Every direct recursive call must advance an actually guarded depth
        // parameter in its own argument slot. One good call cannot excuse an
        // unchanged-depth call (or a step accidentally passed to another arg).
        if facts.bounded_recursive_calls == facts.recursive_calls {
            return;
        }

        self.violations.push(
            RuleViolation::new(
                UNBOUNDED_RECURSION,
                Severity::Error,
                format!(
                    "{UNBOUNDED_RECURSION}: `{fn_name}` directly recurses without an explicit depth bound carried through the recursive call"
                ),
                format!("{fn_name}:{}", sig.ident.span().start().line),
            )
            .with_suggestion(
                "Add a depth/remaining parameter, stop at an explicit bound, and change that parameter monotonically on every recursive call (for example depth + 1 toward MAX_DEPTH or remaining - 1 toward zero)."
                    .to_string(),
            ),
        );
    }
}

impl<'ast> Visit<'ast> for FunctionVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_test = has_cfg_test(&node.attrs);
        if was_test {
            self.test_depth += 1;
        }
        visit::visit_item_mod(self, node);
        if was_test {
            self.test_depth -= 1;
        }
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.check_function(&node.sig, &node.block);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.check_function(&node.sig, &node.block);
    }
}

struct RecursionFacts<'a> {
    fn_name: &'a str,
    depth_params: &'a HashSet<String>,
    param_positions: &'a HashMap<String, usize>,
    recursive_calls: usize,
    bounded_recursive_calls: usize,
    guarded_depth_params: HashSet<String>,
}

impl RecursionFacts<'_> {
    fn comparison_guards(&self, condition: &Expr) -> HashSet<String> {
        let mut collector = GuardCollector {
            depth_params: self.depth_params,
            guarded: HashSet::new(),
        };
        collector.visit_expr(condition);
        collector.guarded
    }

    fn record_recursive_args(
        &mut self,
        args: &syn::punctuated::Punctuated<Expr, syn::Token![,]>,
    ) {
        self.recursive_calls += 1;
        // Require the advancing expression at the declared parameter index;
        // using depth+1 to fill an unrelated argument is not a depth bound.
        if self.depth_params.iter().any(|param| {
            self.guarded_depth_params.contains(param)
                && self
                    .param_positions
                    .get(param)
                    .and_then(|index| args.iter().nth(*index))
                    .is_some_and(|arg| is_monotonic_step(arg, param))
        }) {
            self.bounded_recursive_calls += 1;
        }
    }
}

impl<'ast> Visit<'ast> for RecursionFacts<'_> {
    fn visit_item_fn(&mut self, _node: &'ast syn::ItemFn) {
        // A nested function has its own calls and depth bounds.
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        // Evaluating a comparison does not make the remainder of the
        // function bounded. It constrains a recursive call only while the
        // call is under that branch, or after a branch that returns.
        self.visit_expr(&node.cond);
        let local_guards = self.comparison_guards(&node.cond);
        let earlier_guards = self.guarded_depth_params.clone();

        self.guarded_depth_params.extend(local_guards.iter().cloned());
        self.visit_block(&node.then_branch);
        self.guarded_depth_params = earlier_guards.clone();

        if let Some((_, else_expr)) = &node.else_branch {
            self.guarded_depth_params.extend(local_guards.iter().cloned());
            self.visit_expr(else_expr);
            self.guarded_depth_params = earlier_guards;
        }

        // A comparison followed by a non-exiting branch (e.g. logging)
        // cannot bound a later unconditional self-call.
        if branch_exits_function(&node.then_branch) {
            self.guarded_depth_params.extend(local_guards);
        }
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.visit_expr(&node.cond);
        let local_guards = self.comparison_guards(&node.cond);
        let earlier_guards = self.guarded_depth_params.clone();

        self.guarded_depth_params.extend(local_guards);
        self.visit_block(&node.body);
        // The loop condition is not a guard for recursive calls after it.
        self.guarded_depth_params = earlier_guards;
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if is_direct_self_call(&node.func, self.fn_name) {
            self.record_recursive_args(&node.args);
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == self.fn_name && expr_is_ident(&node.receiver, "self") {
            self.record_recursive_args(&node.args);
        }
        visit::visit_expr_method_call(self, node);
    }
}

struct GuardCollector<'a> {
    depth_params: &'a HashSet<String>,
    guarded: HashSet<String>,
}

impl<'ast> Visit<'ast> for GuardCollector<'_> {
    fn visit_item_fn(&mut self, _node: &'ast syn::ItemFn) {
        // Guards inside a nested function do not bound the enclosing function.
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if is_comparison(&node.op) {
            for param in self.depth_params {
                if expr_mentions_ident(&node.left, param)
                    || expr_mentions_ident(&node.right, param)
                {
                    self.guarded.insert(param.clone());
                }
            }
        }
        visit::visit_expr_binary(self, node);
    }
}

// Only a branch whose final statement returns prevents execution from
// continuing on that path. Ordinary comparisons or logging are not bounds.
fn branch_exits_function(block: &syn::Block) -> bool {
    block.stmts.last().is_some_and(|stmt| match stmt {
        syn::Stmt::Expr(expr, _) => matches!(peel(expr), Expr::Return(_)),
        _ => false,
    })
}

// Method-call arguments omit the self receiver, so index typed parameters
// rather than raw Signature.inputs (which can include self at position zero).
fn typed_parameter_positions(sig: &Signature) -> HashMap<String, usize> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(typed) => Some(typed),
            FnArg::Receiver(_) => None,
        })
        .enumerate()
        .filter_map(|(position, arg)| match arg.pat.as_ref() {
            Pat::Ident(ident) => Some((ident.ident.to_string(), position)),
            _ => None,
        })
        .collect()
}

fn depth_parameter_names(sig: &Signature) -> HashSet<String> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Typed(typed) => match typed.pat.as_ref() {
                Pat::Ident(ident) => {
                    let name = ident.ident.to_string();
                    let lower = name.to_ascii_lowercase();
                    (lower.contains("depth")
                        || lower == "remaining"
                        || lower.ends_with("_remaining")
                        || lower.ends_with("_left"))
                    .then_some(name)
                }
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect()
}

fn is_direct_self_call(expr: &Expr, fn_name: &str) -> bool {
    let Expr::Path(path) = peel(expr) else {
        return false;
    };
    let segments: Vec<_> = path.path.segments.iter().collect();
    match segments.as_slice() {
        [only] => only.ident == fn_name,
        [first, last] => first.ident == "Self" && last.ident == fn_name,
        _ => false,
    }
}

fn is_monotonic_step(expr: &Expr, param: &str) -> bool {
    match peel(expr) {
        Expr::Binary(binary) => match &binary.op {
            syn::BinOp::Add(_) => {
                (expr_is_ident(&binary.left, param) && is_positive_int(&binary.right))
                    || (is_positive_int(&binary.left) && expr_is_ident(&binary.right, param))
            }
            syn::BinOp::Sub(_) => {
                expr_is_ident(&binary.left, param) && is_positive_int(&binary.right)
            }
            _ => false,
        },
        Expr::MethodCall(call) => {
            let method = call.method.to_string();
            matches!(
                method.as_str(),
                "saturating_add" | "saturating_sub" | "checked_add" | "checked_sub"
            ) && expr_is_ident(&call.receiver, param)
                && call.args.first().is_some_and(is_positive_int)
        }
        _ => false,
    }
}

fn is_positive_int(expr: &Expr) -> bool {
    match peel(expr) {
        Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Int(value) => value.base10_parse::<u64>().is_ok_and(|value| value > 0),
            _ => false,
        },
        _ => false,
    }
}

fn expr_is_ident(expr: &Expr, name: &str) -> bool {
    match peel(expr) {
        Expr::Path(path) => path.path.is_ident(name),
        _ => false,
    }
}

fn expr_mentions_ident(expr: &Expr, name: &str) -> bool {
    struct IdentUse<'a> {
        name: &'a str,
        found: bool,
    }

    impl<'ast> Visit<'ast> for IdentUse<'_> {
        fn visit_item_fn(&mut self, _node: &'ast syn::ItemFn) {
            // A nested function's identifiers are outside this expression.
        }

        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if node.path.is_ident(self.name) {
                self.found = true;
                return;
            }
            visit::visit_expr_path(self, node);
        }
    }

    let mut visitor = IdentUse { name, found: false };
    visitor.visit_expr(expr);
    visitor.found
}

fn peel(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => peel(&paren.expr),
        Expr::Group(group) => peel(&group.expr),
        _ => expr,
    }
}

fn is_comparison(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::Eq(_)
            | syn::BinOp::Ne(_)
            | syn::BinOp::Lt(_)
            | syn::BinOp::Le(_)
            | syn::BinOp::Gt(_)
            | syn::BinOp::Ge(_)
    )
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        match &attr.meta {
            syn::Meta::List(list) => list
                .tokens
                .to_string()
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .any(|part| part == "test"),
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_nested_functions_do_not_change_enclosing_recursion_facts() {
        let source = r#"
fn clean_outer() {
    fn unused() {
        clean_outer();
    }
}

fn poisoned_outer(depth: u32) {
    fn unused(depth: u32) {
        if depth >= 8 {
            return;
        }
        poisoned_outer(depth + 1);
    }
    poisoned_outer(depth);
}

fn condition_guard(depth: u32) {
    if {
        fn unused(depth: u32) {
            if depth < 10 {}
        }
        true
    } {
        condition_guard(depth + 1);
    }
}

fn condition_ident(depth: u32) {
    if ({
        fn unused(depth: u32) {
            let _ = depth;
        }
        1
    } < 10) {
        condition_ident(depth + 1);
    }
}

fn bounded_control(depth: u32) {
    if depth >= 8 {
        return;
    }
    bounded_control(depth + 1);
}
"#;
        syn::parse_str::<syn::File>(source).expect("valid nested-function fixture");
        let findings = UnboundedRecursionRule.check(source);
        let owners: Vec<_> = findings
            .iter()
            .map(|finding| finding.location.split_once(':').unwrap().0)
            .collect();
        assert_eq!(
            owners,
            vec!["poisoned_outer", "condition_guard", "condition_ident"],
            "nested helper facts must not create or hide enclosing findings: {findings:#?}"
        );
        for finding in findings {
            assert_eq!(finding.rule_name, UNBOUNDED_RECURSION);
            assert_eq!(finding.severity, Severity::Error);
        }
    }

    #[test]
    fn every_recursive_call_must_step_the_guarded_argument_in_its_own_slot() {
        let source = r#"
fn mixed_calls(depth: u32) {
    if depth >= 8 { return; }
    mixed_calls(depth + 1);
    mixed_calls(depth);
}

fn wrong_argument(depth: u32, value: u32) {
    if depth >= 8 { return; }
    wrong_argument(depth, depth + 1);
}

fn fully_bounded(depth: u32) {
    if depth >= 8 { return; }
    fully_bounded(depth + 1);
}
"#;
        let findings = UnboundedRecursionRule.check(source);
        let names: Vec<_> = findings
            .iter()
            .map(|finding| finding.location.split_once(':').unwrap().0)
            .collect();
        assert_eq!(names, vec!["mixed_calls", "wrong_argument"]);
    }
    #[test]
    fn unrelated_depth_checks_do_not_bound_later_recursive_calls() {
        let source = r#"
fn checked_but_not_stopped(depth: u32) {
    if depth >= 8 {
        let _message = "still running";
    }
    checked_but_not_stopped(depth + 1);
}

fn checked_in_loop_only(depth: u32) {
    while depth >= 8 { break; }
    checked_in_loop_only(depth + 1);
}

fn nested_nonterminal_check(depth: u32) {
    if depth < 8 {
        if depth >= 100 { return; }
    }
    nested_nonterminal_check(depth + 1);
}

fn recursive_only_under_bound(depth: u32) {
    if depth < 8 {
        recursive_only_under_bound(depth + 1);
    }
}

fn terminating_guard(depth: u32) {
    if depth >= 8 { return; }
    terminating_guard(depth + 1);
}

fn terminating_guard_else(depth: u32) {
    if depth >= 8 { return; } else {
        terminating_guard_else(depth + 1);
    }
}
"#;
        syn::parse_str::<syn::File>(source).expect("valid guard-scope fixture");
        let findings = UnboundedRecursionRule.check(source);
        let names: Vec<_> = findings
            .iter()
            .map(|finding| finding.location.split_once(':').unwrap().0)
            .collect();
        assert_eq!(
            names,
            vec![
                "checked_but_not_stopped",
                "checked_in_loop_only",
                "nested_nonterminal_check",
            ],
            "only comparisons that constrain the recursion path may bound it: {findings:#?}"
        );
    }


}
