use crate::finding_codes::LOOP_ACCUMULATOR;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashMap;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{BinOp, Expr, Pat};

/// Finds unchecked self-addition to bindings that survive a loop iteration.
/// This is a syntactic warning, not a proof that an integer value overflows.
pub struct LoopAccumulatorRule;

impl LoopAccumulatorRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoopAccumulatorRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for LoopAccumulatorRule {
    fn name(&self) -> &str {
        "loop_accumulator"
    }

    fn description(&self) -> &str {
        "Detects unchecked addition to a loop-carried accumulator"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Some(file) = crate::parse_cache::parse_cached(source) else {
            return Vec::new();
        };
        let mut visitor = AccumulatorVisitor::default();
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct AccumulatorVisitor {
    violations: Vec<RuleViolation>,
    scopes: Vec<HashMap<String, usize>>,
    // Each loop remembers the first binding ID created after it was entered.
    // A resolved binding with a smaller ID survives that loop's iterations.
    loops: Vec<usize>,
    next_binding: usize,
    current_fn: Option<String>,
}

impl AccumulatorVisitor {
    fn bind_pattern(&mut self, pat: &Pat) {
        struct Names(Vec<String>);
        impl<'ast> Visit<'ast> for Names {
            fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
                self.0.push(pat.ident.to_string());
                visit::visit_pat_ident(self, pat);
            }
        }
        let mut names = Names(Vec::new());
        names.visit_pat(pat);
        for name in names.0 {
            if let Some(scope) = self.scopes.last_mut() {
                scope.insert(name, self.next_binding);
                self.next_binding += 1;
            }
        }
    }

    fn visit_function(&mut self, signature: &syn::Signature, block: &syn::Block) {
        let old_scopes = std::mem::take(&mut self.scopes);
        let old_loops = std::mem::take(&mut self.loops);
        let old_fn = self.current_fn.replace(signature.ident.to_string());
        self.scopes.push(HashMap::new());
        for argument in &signature.inputs {
            if let syn::FnArg::Typed(argument) = argument {
                self.bind_pattern(&argument.pat);
            }
        }
        self.visit_block(block);
        self.scopes = old_scopes;
        self.loops = old_loops;
        self.current_fn = old_fn;
    }

    fn record(&mut self, target: &Expr) {
        let Some(name) = ident_of(target) else {
            return;
        };
        let binding = self.scopes.iter().rev().find_map(|scope| scope.get(&name));
        let Some(binding) = binding else {
            return;
        };
        if !self.loops.iter().any(|first_local| binding < first_local) {
            return;
        }
        let Some(function) = &self.current_fn else {
            return;
        };
        self.violations.push(
            RuleViolation::new(
                LOOP_ACCUMULATOR,
                Severity::Warning,
                format!(
                    "Unchecked addition updates loop-carried accumulator `{name}` and may overflow"
                ),
                format!("{function}:{}", target.span().start().line),
            )
            .with_suggestion(format!(
                "Use `{name}.checked_add(value)` with explicit overflow handling, \
                 or `{name}.saturating_add(value)` if clamping is intended"
            )),
        );
    }
}

impl<'ast> Visit<'ast> for AccumulatorVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.visit_function(&node.sig, &node.block);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.visit_function(&node.sig, &node.block);
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        if let Some(block) = &node.default {
            self.visit_function(&node.sig, block);
        }
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.scopes.push(HashMap::new());
        visit::visit_block(self, node);
        self.scopes.pop();
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        // A shadowing binding is not visible in its initializer or else block.
        if let Some(initializer) = &node.init {
            self.visit_expr(&initializer.expr);
            if let Some((_, otherwise)) = &initializer.diverge {
                self.visit_expr(otherwise);
            }
        }
        self.bind_pattern(&node.pat);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        // The iterable is evaluated once, before this loop is active.
        self.visit_expr(&node.expr);
        self.loops.push(self.next_binding);
        self.scopes.push(HashMap::new());
        self.bind_pattern(&node.pat);
        self.visit_block(&node.body);
        self.scopes.pop();
        self.loops.pop();
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.loops.push(self.next_binding);
        self.scopes.push(HashMap::new());
        // Unlike a for iterator, a while condition executes each iteration.
        self.visit_expr(&node.cond);
        self.visit_block(&node.body);
        self.scopes.pop();
        self.loops.pop();
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.loops.push(self.next_binding);
        self.visit_block(&node.body);
        self.loops.pop();
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.scopes.push(HashMap::new());
        self.visit_expr(&node.cond);
        self.visit_block(&node.then_branch);
        self.scopes.pop();
        if let Some((_, otherwise)) = &node.else_branch {
            self.visit_expr(otherwise);
        }
    }

    fn visit_expr_let(&mut self, node: &'ast syn::ExprLet) {
        self.visit_expr(&node.expr);
        self.bind_pattern(&node.pat);
    }

    fn visit_arm(&mut self, node: &'ast syn::Arm) {
        self.scopes.push(HashMap::new());
        self.bind_pattern(&node.pat);
        if let Some((_, guard)) = &node.guard {
            self.visit_expr(guard);
        }
        self.visit_expr(&node.body);
        self.scopes.pop();
    }

    fn visit_expr_closure(&mut self, node: &'ast syn::ExprClosure) {
        // Declaring a deferred body inside a loop does not execute it there.
        // Captured bindings remain visible to loops inside the deferred body.
        let old_loops = std::mem::take(&mut self.loops);
        self.scopes.push(HashMap::new());
        for input in &node.inputs {
            self.bind_pattern(input);
        }
        self.visit_expr(&node.body);
        self.scopes.pop();
        self.loops = old_loops;
    }

    fn visit_expr_async(&mut self, node: &'ast syn::ExprAsync) {
        let old_loops = std::mem::take(&mut self.loops);
        self.visit_block(&node.block);
        self.loops = old_loops;
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, BinOp::AddAssign(_)) {
            self.record(&node.left);
        }
        visit::visit_expr_binary(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if let (Some(name), Expr::Binary(add)) = (ident_of(&node.left), ungroup(&node.right)) {
            if matches!(add.op, BinOp::Add(_)) && has_addend(&node.right, &name) {
                self.record(&node.left);
            }
        }
        visit::visit_expr_assign(self, node);
    }
}

fn ungroup(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(node) => ungroup(&node.expr),
        Expr::Group(node) => ungroup(&node.expr),
        _ => expr,
    }
}

fn ident_of(expr: &Expr) -> Option<String> {
    match ungroup(expr) {
        Expr::Path(node) => node.path.get_ident().map(ToString::to_string),
        _ => None,
    }
}

// Match only an addition tree with the accumulator as an actual addend.
// A checked_add/saturating_add receiver is not unchecked self-addition.
fn has_addend(expr: &Expr, name: &str) -> bool {
    match ungroup(expr) {
        Expr::Path(node) => node.path.is_ident(name),
        Expr::Binary(node) if matches!(node.op, BinOp::Add(_)) => {
            has_addend(&node.left, name) || has_addend(&node.right, name)
        }
        _ => false,
    }
}
