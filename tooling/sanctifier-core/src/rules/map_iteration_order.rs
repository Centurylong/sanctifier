use crate::finding_codes::MAP_ITERATION_ORDER;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, FnArg, Pat, Type};

/// Advises when a Soroban `Map` is consumed in a way where iteration order can
/// select the returned or persisted outcome.
///
/// Full traversals are intentionally ignored: this rule is about selection and
/// short-circuit behavior, not the mere presence of `Map::iter()`.
pub struct MapIterationOrderRule;

impl MapIterationOrderRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MapIterationOrderRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for MapIterationOrderRule {
    fn name(&self) -> &str {
        "map_iteration_order"
    }

    fn description(&self) -> &str {
        "Detects outcomes selected from Soroban Map iteration order"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Some(file) = crate::parse_cache::parse_cached(source) else {
            return Vec::new();
        };

        let mut visitor = MapOrderVisitor::default();
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct MapOrderVisitor {
    violations: Vec<RuleViolation>,
    scopes: Vec<HashMap<String, bool>>,
    current_fn: Option<String>,
}

impl MapOrderVisitor {
    fn visit_function(&mut self, signature: &syn::Signature, block: &syn::Block) {
        let old_scopes = std::mem::take(&mut self.scopes);
        let old_fn = self.current_fn.replace(signature.ident.to_string());

        self.scopes.push(HashMap::new());
        for argument in &signature.inputs {
            if let FnArg::Typed(argument) = argument {
                self.bind_pattern(&argument.pat, type_is_map(&argument.ty));
            }
        }

        self.visit_block(block);
        self.scopes = old_scopes;
        self.current_fn = old_fn;
    }

    fn bind_pattern(&mut self, pat: &Pat, is_map: bool) {
        let names = pattern_names(pat);
        if let Some(scope) = self.scopes.last_mut() {
            for name in names {
                scope.insert(name, is_map);
            }
        }
    }

    fn is_map_binding(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
            .unwrap_or(false)
    }

    fn map_iteration_source(&self, expr: &Expr) -> Option<String> {
        let Expr::MethodCall(call) = ungroup(expr) else {
            return None;
        };
        let method = call.method.to_string();

        if matches!(method.as_str(), "iter" | "keys" | "values") {
            let name = simple_ident(&call.receiver)?;
            if self.is_map_binding(&name) {
                return Some(format!("{name}.{method}()"));
            }
            return None;
        }

        if matches!(
            method.as_str(),
            "enumerate" | "filter" | "filter_map" | "map" | "skip" | "take" | "peekable"
        ) {
            return self.map_iteration_source(&call.receiver);
        }

        None
    }

    fn record_selector(&mut self, source: String, method: &str, line: usize) {
        let Some(function) = &self.current_fn else {
            return;
        };

        self.violations.push(
            RuleViolation::new(
                MAP_ITERATION_ORDER,
                Severity::Warning,
                format!(
                    "{MAP_ITERATION_ORDER}: `{function}` selects from `{source}` with `{method}`, so the outcome depends on Map iteration order"
                ),
                format!("{function}:{line}"),
            )
            .with_suggestion(
                "Select by an explicit stable key/order, or use a keyed lookup or order-independent aggregation"
                    .to_string(),
            ),
        );
    }

    fn record_short_circuit(&mut self, source: String, line: usize) {
        let Some(function) = &self.current_fn else {
            return;
        };

        self.violations.push(
            RuleViolation::new(
                MAP_ITERATION_ORDER,
                Severity::Warning,
                format!(
                    "{MAP_ITERATION_ORDER}: `{function}` short-circuits `{source}` after consuming a Map item, so which item determines the outcome depends on iteration order"
                ),
                format!("{function}:{line}"),
            )
            .with_suggestion(
                "Sort by an explicit stable key before selecting an item, or traverse the full Map with order-independent logic"
                    .to_string(),
            ),
        );
    }
}

impl<'ast> Visit<'ast> for MapOrderVisitor {
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
        if let Some(initializer) = &node.init {
            self.visit_expr(&initializer.expr);
            if let Some((_, otherwise)) = &initializer.diverge {
                self.visit_expr(otherwise);
            }
        }

        let is_map = match &node.pat {
            Pat::Type(typed) => type_is_map(&typed.ty),
            _ => node
                .init
                .as_ref()
                .is_some_and(|initializer| expr_is_map_constructor(&initializer.expr)),
        };
        self.bind_pattern(&node.pat, is_map);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.visit_expr(&node.expr);

        if let Some(source) = self.map_iteration_source(&node.expr) {
            let bindings = pattern_names(&node.pat).into_iter().collect::<HashSet<_>>();
            let mut outcome = EarlyOutcomeVisitor::new(bindings);
            outcome.visit_block(&node.body);
            if outcome.is_order_dependent() {
                self.record_short_circuit(source, node.expr.span().start().line);
            }
        }

        self.scopes.push(HashMap::new());
        self.bind_pattern(&node.pat, false);
        self.visit_block(&node.body);
        self.scopes.pop();
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if matches!(
            method.as_str(),
            "next" | "last" | "nth" | "find" | "find_map" | "position"
        ) {
            if let Some(source) = self.map_iteration_source(&node.receiver) {
                self.record_selector(source, &method, node.method.span().start().line);
            }
        } else if method == "get"
            && node.args.first().is_some_and(is_zero_literal)
            && self.map_iteration_source(&node.receiver).is_some()
        {
            let source = self.map_iteration_source(&node.receiver).unwrap();
            self.record_selector(source, &method, node.method.span().start().line);
        }

        visit::visit_expr_method_call(self, node);
    }
}

struct EarlyOutcomeVisitor {
    loop_bindings: HashSet<String>,
    assigned_from_binding: bool,
    breaks_outer_loop: bool,
    returns_binding: bool,
    nested_loop_depth: usize,
}

impl EarlyOutcomeVisitor {
    fn new(loop_bindings: HashSet<String>) -> Self {
        Self {
            loop_bindings,
            assigned_from_binding: false,
            breaks_outer_loop: false,
            returns_binding: false,
            nested_loop_depth: 0,
        }
    }

    fn is_order_dependent(&self) -> bool {
        self.returns_binding || (self.assigned_from_binding && self.breaks_outer_loop)
    }
}

impl<'ast> Visit<'ast> for EarlyOutcomeVisitor {
    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        if expr_references_any(&node.right, &self.loop_bindings) {
            self.assigned_from_binding = true;
        }
        visit::visit_expr_assign(self, node);
    }

    fn visit_expr_break(&mut self, _node: &'ast syn::ExprBreak) {
        if self.nested_loop_depth == 0 {
            self.breaks_outer_loop = true;
        }
    }

    fn visit_expr_return(&mut self, node: &'ast syn::ExprReturn) {
        if node
            .expr
            .as_ref()
            .is_some_and(|expr| expr_references_any(expr, &self.loop_bindings))
        {
            self.returns_binding = true;
        }
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        self.nested_loop_depth += 1;
        visit::visit_expr_for_loop(self, node);
        self.nested_loop_depth -= 1;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.nested_loop_depth += 1;
        visit::visit_expr_while(self, node);
        self.nested_loop_depth -= 1;
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.nested_loop_depth += 1;
        visit::visit_expr_loop(self, node);
        self.nested_loop_depth -= 1;
    }

    fn visit_expr_closure(&mut self, _node: &'ast syn::ExprClosure) {}

    fn visit_expr_async(&mut self, _node: &'ast syn::ExprAsync) {}
}

fn pattern_names(pat: &Pat) -> Vec<String> {
    struct Names(Vec<String>);

    impl<'ast> Visit<'ast> for Names {
        fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
            self.0.push(pat.ident.to_string());
            visit::visit_pat_ident(self, pat);
        }
    }

    let mut names = Names(Vec::new());
    names.visit_pat(pat);
    names.0
}

fn type_is_map(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Map"),
        Type::Reference(reference) => type_is_map(&reference.elem),
        Type::Paren(paren) => type_is_map(&paren.elem),
        Type::Group(group) => type_is_map(&group.elem),
        _ => false,
    }
}

fn expr_is_map_constructor(expr: &Expr) -> bool {
    match ungroup(expr) {
        Expr::Call(call) => match ungroup(&call.func) {
            Expr::Path(path) => path.path.segments.iter().any(|segment| segment.ident == "Map"),
            _ => false,
        },
        _ => false,
    }
}

fn simple_ident(expr: &Expr) -> Option<String> {
    match ungroup(expr) {
        Expr::Path(path) => path.path.get_ident().map(ToString::to_string),
        Expr::Reference(reference) => simple_ident(&reference.expr),
        _ => None,
    }
}

fn ungroup(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(paren) => ungroup(&paren.expr),
        Expr::Group(group) => ungroup(&group.expr),
        _ => expr,
    }
}

fn is_zero_literal(expr: &Expr) -> bool {
    matches!(
        ungroup(expr),
        Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Int(value) if value.base10_digits() == "0")
    )
}

fn expr_references_any(expr: &Expr, names: &HashSet<String>) -> bool {
    struct RefVisitor<'a> {
        names: &'a HashSet<String>,
        found: bool,
    }

    impl<'ast> Visit<'ast> for RefVisitor<'_> {
        fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
            if path
                .path
                .get_ident()
                .is_some_and(|ident| self.names.contains(&ident.to_string()))
            {
                self.found = true;
                return;
            }
            visit::visit_expr_path(self, path);
        }
    }

    let mut visitor = RefVisitor {
        names,
        found: false,
    };
    visitor.visit_expr(expr);
    visitor.found
}
