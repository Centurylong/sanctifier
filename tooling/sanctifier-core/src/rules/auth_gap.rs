use crate::finding_codes::SANCT_VISIBILITY;
use crate::rules::{Patch, Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{parse_str, File, Item};

pub struct AuthGapRule;

impl AuthGapRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AuthGapRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for AuthGapRule {
    fn name(&self) -> &str {
        "auth_gap"
    }

    fn description(&self) -> &str {
        "Detects public functions that perform storage mutations without authentication checks"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut gaps = Vec::new();
        for item in &file.items {
            if let Item::Impl(i) = item {
                for impl_item in &i.items {
                    if let syn::ImplItem::Fn(f) = impl_item {
                        if let syn::Visibility::Public(_) = f.vis {
                            let fn_name = f.sig.ident.to_string();
                            let mut has_mutation = false;
                            let mut has_read = false;
                            let mut has_auth = false;
                            check_fn_body(
                                &f.block,
                                &mut has_mutation,
                                &mut has_read,
                                &mut has_auth,
                            );
                            if has_mutation && !has_read && !has_auth {
                                gaps.push(RuleViolation::new(
                                    self.name(),
                                    Severity::Warning,
                                    format!("Function '{}' performs storage mutation without authentication", fn_name),
                                    fn_name.clone(),
                                ).with_suggestion("Add require_auth() or require_auth_for_args() before storage operations".to_string()));
                            }
                        }
                    }
                }
            }
        }
        gaps
    }

    fn fix(&self, source: &str) -> Vec<Patch> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut patches = Vec::new();
        for item in &file.items {
            if let Item::Impl(i) = item {
                for impl_item in &i.items {
                    if let syn::ImplItem::Fn(f) = impl_item {
                        if let syn::Visibility::Public(_) = f.vis {
                            let mut has_mutation = false;
                            let mut has_read = false;
                            let mut has_auth = false;
                            check_fn_body(
                                &f.block,
                                &mut has_mutation,
                                &mut has_read,
                                &mut has_auth,
                            );
                            if has_mutation && !has_read && !has_auth {
                                // Add require_auth() as the first statement in the function
                                if let Some(first_stmt) = f.block.stmts.first() {
                                    let span = first_stmt.span();
                                    patches.push(Patch {
                                        start_line: span.start().line,
                                        start_column: span.start().column,
                                        end_line: span.start().line,
                                        end_column: span.start().column,
                                        replacement: "env.require_auth();\n    ".to_string(),
                                        description: format!(
                                            "Add require_auth() to function '{}'",
                                            f.sig.ident
                                        ),
                                    });
                                } else {
                                    // Empty body, just insert at the start of block
                                    let span = f.block.span();
                                    patches.push(Patch {
                                        start_line: span.start().line,
                                        start_column: span.start().column + 1,
                                        end_line: span.start().line,
                                        end_column: span.start().column + 1,
                                        replacement: "\n        env.require_auth();".to_string(),
                                        description: format!(
                                            "Add require_auth() to function '{}'",
                                            f.sig.ident
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        patches
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Flags helper-shaped state mutators that are accidentally exported as
/// Soroban contract entrypoints without an authorization guard.
pub struct VisibilityLeakRule;

impl VisibilityLeakRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VisibilityLeakRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for VisibilityLeakRule {
    fn name(&self) -> &str {
        "sanct_visibility"
    }

    fn description(&self) -> &str {
        "Detects public helper-shaped #[contractimpl] methods that mutate state without authorization"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let program = VisibilityProgram::from_file(&file);
        program
            .entrypoints
            .iter()
            .copied()
            .filter(|function| program.has_unauthenticated_mutation(function))
            .map(|function| {
                let function_name = function.sig.ident.to_string();
                RuleViolation::new(
                    SANCT_VISIBILITY,
                    Severity::Warning,
                    format!(
                        "{SANCT_VISIBILITY}: public helper-shaped function \
                         `{function_name}` mutates contract state without authorization"
                    ),
                    function_name,
                )
                .with_suggestion(
                    "Make the helper private or require authorization before mutating state"
                        .to_string(),
                )
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct VisibilityProgram<'ast> {
    functions: HashMap<String, Vec<VisibilityFunction<'ast>>>,
    entrypoints: Vec<&'ast syn::ImplItemFn>,
}

struct VisibilityFunction<'ast> {
    block: &'ast syn::Block,
    parameters: Vec<Option<String>>,
}

impl<'ast> VisibilityProgram<'ast> {
    fn from_file(file: &'ast File) -> Self {
        let mut collector = VisibilityCollector::default();
        collector.visit_file(file);
        collector.program
    }

    fn has_unauthenticated_mutation(&self, function: &'ast syn::ImplItemFn) -> bool {
        let mut call_stack = HashSet::new();
        self.analyze_function(
            &function.block,
            VisibilityFlow::unauthenticated(),
            HashSet::new(),
            &mut call_stack,
        )
        .unsafe_mutation
    }

    fn analyze_function(
        &self,
        block: &'ast syn::Block,
        flow: VisibilityFlow,
        aliases: HashSet<String>,
        call_stack: &mut HashSet<usize>,
    ) -> VisibilityFlow {
        let function_id = block as *const syn::Block as usize;
        if !call_stack.insert(function_id) {
            return flow;
        }

        let result = self.analyze_block(block, flow, &aliases, call_stack);
        call_stack.remove(&function_id);
        result.return_to_caller()
    }

    fn analyze_block(
        &self,
        block: &'ast syn::Block,
        mut flow: VisibilityFlow,
        parent_aliases: &HashSet<String>,
        call_stack: &mut HashSet<usize>,
    ) -> VisibilityFlow {
        let mut aliases = parent_aliases.clone();
        for statement in &block.stmts {
            if flow.states == 0 {
                break;
            }

            flow = match statement {
                syn::Stmt::Local(local) => {
                    let analyzed = local.init.as_ref().map_or(flow, |init| {
                        self.analyze_expr(&init.expr, flow, &aliases, call_stack)
                    });
                    if let Some(binding) = pattern_identifier(&local.pat) {
                        let is_storage = local
                            .init
                            .as_ref()
                            .is_some_and(|init| expr_is_storage_handle(&init.expr, &aliases));
                        if is_storage {
                            aliases.insert(binding);
                        } else {
                            aliases.remove(&binding);
                        }
                    }
                    analyzed
                }
                syn::Stmt::Expr(expr, _) => self.analyze_expr(expr, flow, &aliases, call_stack),
                syn::Stmt::Macro(statement) if is_auth_path(&statement.mac.path) => {
                    flow.authenticated()
                }
                syn::Stmt::Item(_) | syn::Stmt::Macro(_) => flow,
            };
        }

        flow
    }

    fn analyze_expr(
        &self,
        expression: &'ast syn::Expr,
        flow: VisibilityFlow,
        aliases: &HashSet<String>,
        call_stack: &mut HashSet<usize>,
    ) -> VisibilityFlow {
        match expression {
            syn::Expr::Call(call) => {
                let storage_arguments = call
                    .args
                    .iter()
                    .map(|argument| expr_is_storage_handle(argument, aliases))
                    .collect::<Vec<_>>();
                let flow = call.args.iter().fold(flow, |current, argument| {
                    self.analyze_expr(argument, current, aliases, call_stack)
                });
                let Some(name) = path_name(&call.func) else {
                    return flow;
                };

                if is_auth_call(&name) {
                    flow.authenticated()
                } else if self.functions.contains_key(&name) {
                    self.analyze_local_call(&name, &storage_arguments, flow, call_stack)
                } else if is_qualified_storage_mutation(&call.func) {
                    flow.with_mutation()
                } else {
                    flow
                }
            }
            syn::Expr::MethodCall(call) => {
                let storage_arguments =
                    std::iter::once(expr_is_storage_handle(&call.receiver, aliases))
                        .chain(
                            call.args
                                .iter()
                                .map(|argument| expr_is_storage_handle(argument, aliases)),
                        )
                        .collect::<Vec<_>>();
                let flow = self.analyze_expr(&call.receiver, flow, aliases, call_stack);
                let flow = call.args.iter().fold(flow, |current, argument| {
                    self.analyze_expr(argument, current, aliases, call_stack)
                });
                let method = call.method.to_string();

                if is_auth_call(&method) {
                    flow.authenticated()
                } else if is_storage_mutation_method(&method)
                    && expr_is_storage_handle(&call.receiver, aliases)
                {
                    flow.with_mutation()
                } else if is_storage_access_method(&method) {
                    flow
                } else {
                    self.analyze_local_call(&method, &storage_arguments, flow, call_stack)
                }
            }
            syn::Expr::If(branch) => {
                let condition = self.analyze_expr(&branch.cond, flow, aliases, call_stack);
                let then_flow =
                    self.analyze_block(&branch.then_branch, condition, aliases, call_stack);
                let else_flow = branch.else_branch.as_ref().map_or(condition, |(_, body)| {
                    self.analyze_expr(body, condition, aliases, call_stack)
                });
                then_flow.merge(else_flow)
            }
            syn::Expr::Match(branch) => {
                let scrutinee = self.analyze_expr(&branch.expr, flow, aliases, call_stack);
                branch
                    .arms
                    .iter()
                    .map(|arm| {
                        let guarded = arm.guard.as_ref().map_or(scrutinee, |(_, guard)| {
                            self.analyze_expr(guard, scrutinee, aliases, call_stack)
                        });
                        self.analyze_expr(&arm.body, guarded, aliases, call_stack)
                    })
                    .reduce(VisibilityFlow::merge)
                    .unwrap_or(scrutinee)
            }
            syn::Expr::Block(block) => self.analyze_block(&block.block, flow, aliases, call_stack),
            syn::Expr::TryBlock(block) => {
                self.analyze_block(&block.block, flow, aliases, call_stack)
            }
            syn::Expr::Unsafe(block) => self.analyze_block(&block.block, flow, aliases, call_stack),
            syn::Expr::Const(block) => self.analyze_block(&block.block, flow, aliases, call_stack),
            syn::Expr::ForLoop(loop_expr) => {
                let (loop_flow, outer_breaks, outer_continues) = flow.enter_loop();
                let before_body =
                    self.analyze_expr(&loop_expr.expr, loop_flow, aliases, call_stack);
                let body = self.analyze_block(&loop_expr.body, before_body, aliases, call_stack);
                before_body
                    .merge(body)
                    .finish_optional_loop()
                    .restore_outer_loop(outer_breaks, outer_continues)
            }
            syn::Expr::While(loop_expr) => {
                let (loop_flow, outer_breaks, outer_continues) = flow.enter_loop();
                let condition = self.analyze_expr(&loop_expr.cond, loop_flow, aliases, call_stack);
                let body = self.analyze_block(&loop_expr.body, condition, aliases, call_stack);
                condition
                    .merge(body)
                    .finish_optional_loop()
                    .restore_outer_loop(outer_breaks, outer_continues)
            }
            syn::Expr::Loop(loop_expr) => {
                let (loop_flow, outer_breaks, outer_continues) = flow.enter_loop();
                self.analyze_block(&loop_expr.body, loop_flow, aliases, call_stack)
                    .finish_mandatory_loop()
                    .restore_outer_loop(outer_breaks, outer_continues)
            }
            syn::Expr::Return(return_expr) => {
                let mut result = return_expr.expr.as_ref().map_or(flow, |value| {
                    self.analyze_expr(value, flow, aliases, call_stack)
                });
                result.return_states |= result.states;
                result.states = 0;
                result
            }
            syn::Expr::Break(break_expr) => {
                let mut result = break_expr.expr.as_ref().map_or(flow, |value| {
                    self.analyze_expr(value, flow, aliases, call_stack)
                });
                result.break_states |= result.states;
                result.states = 0;
                result
            }
            syn::Expr::Macro(macro_expr) if is_auth_path(&macro_expr.mac.path) => {
                flow.authenticated()
            }
            syn::Expr::Array(array) => {
                self.analyze_many(array.elems.iter(), flow, aliases, call_stack)
            }
            syn::Expr::Tuple(tuple) => {
                self.analyze_many(tuple.elems.iter(), flow, aliases, call_stack)
            }
            syn::Expr::Binary(binary) => {
                let left = self.analyze_expr(&binary.left, flow, aliases, call_stack);
                let right = self.analyze_expr(&binary.right, left, aliases, call_stack);
                if matches!(binary.op, syn::BinOp::And(_) | syn::BinOp::Or(_)) {
                    left.merge(right)
                } else {
                    right
                }
            }
            syn::Expr::Assign(assign) => {
                let left = self.analyze_expr(&assign.left, flow, aliases, call_stack);
                self.analyze_expr(&assign.right, left, aliases, call_stack)
            }
            syn::Expr::Index(index) => {
                let receiver = self.analyze_expr(&index.expr, flow, aliases, call_stack);
                self.analyze_expr(&index.index, receiver, aliases, call_stack)
            }
            syn::Expr::Struct(struct_expr) => {
                let fields = self.analyze_many(
                    struct_expr.fields.iter().map(|field| &field.expr),
                    flow,
                    aliases,
                    call_stack,
                );
                struct_expr.rest.as_ref().map_or(fields, |rest| {
                    self.analyze_expr(rest, fields, aliases, call_stack)
                })
            }
            syn::Expr::Range(range) => {
                let start = range.start.as_ref().map_or(flow, |start| {
                    self.analyze_expr(start, flow, aliases, call_stack)
                });
                range.end.as_ref().map_or(start, |end| {
                    self.analyze_expr(end, start, aliases, call_stack)
                })
            }
            syn::Expr::Repeat(repeat) => {
                let value = self.analyze_expr(&repeat.expr, flow, aliases, call_stack);
                self.analyze_expr(&repeat.len, value, aliases, call_stack)
            }
            syn::Expr::Let(let_expr) => {
                self.analyze_expr(&let_expr.expr, flow, aliases, call_stack)
            }
            syn::Expr::Await(await_expr) => {
                self.analyze_expr(&await_expr.base, flow, aliases, call_stack)
            }
            syn::Expr::Cast(cast) => self.analyze_expr(&cast.expr, flow, aliases, call_stack),
            syn::Expr::Field(field) => self.analyze_expr(&field.base, flow, aliases, call_stack),
            syn::Expr::Group(group) => self.analyze_expr(&group.expr, flow, aliases, call_stack),
            syn::Expr::Paren(paren) => self.analyze_expr(&paren.expr, flow, aliases, call_stack),
            syn::Expr::Reference(reference) => {
                self.analyze_expr(&reference.expr, flow, aliases, call_stack)
            }
            syn::Expr::Try(try_expr) => {
                self.analyze_expr(&try_expr.expr, flow, aliases, call_stack)
            }
            syn::Expr::Unary(unary) => self.analyze_expr(&unary.expr, flow, aliases, call_stack),
            syn::Expr::Yield(yield_expr) => yield_expr.expr.as_ref().map_or(flow, |value| {
                self.analyze_expr(value, flow, aliases, call_stack)
            }),
            syn::Expr::Async(_)
            | syn::Expr::Closure(_)
            | syn::Expr::Infer(_)
            | syn::Expr::Lit(_)
            | syn::Expr::Macro(_)
            | syn::Expr::Path(_)
            | syn::Expr::Verbatim(_) => flow,
            syn::Expr::Continue(_) => flow.with_continue(),
            _ => flow,
        }
    }

    fn analyze_many<'expr>(
        &self,
        expressions: impl Iterator<Item = &'expr syn::Expr>,
        flow: VisibilityFlow,
        aliases: &HashSet<String>,
        call_stack: &mut HashSet<usize>,
    ) -> VisibilityFlow
    where
        'ast: 'expr,
    {
        expressions.fold(flow, |current, expression| {
            self.analyze_expr(expression, current, aliases, call_stack)
        })
    }

    fn analyze_local_call(
        &self,
        name: &str,
        storage_arguments: &[bool],
        flow: VisibilityFlow,
        call_stack: &mut HashSet<usize>,
    ) -> VisibilityFlow {
        let Some(functions) = self.functions.get(name) else {
            return flow;
        };

        functions
            .iter()
            .map(|function| {
                let aliases = function
                    .parameters
                    .iter()
                    .zip(storage_arguments)
                    .filter_map(
                        |(parameter, is_storage)| {
                            if *is_storage {
                                parameter.clone()
                            } else {
                                None
                            }
                        },
                    )
                    .collect();
                self.analyze_function(function.block, flow.enter_callee(), aliases, call_stack)
                    .restore_caller_exits(flow)
            })
            .reduce(VisibilityFlow::merge)
            .unwrap_or(flow)
    }
}

#[derive(Default)]
struct VisibilityCollector<'ast> {
    program: VisibilityProgram<'ast>,
}

impl<'ast> Visit<'ast> for VisibilityCollector<'ast> {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if has_cfg_test(&module.attrs) {
            return;
        }
        visit::visit_item_mod(self, module);
    }

    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        let is_contract_impl = has_attr(&item_impl.attrs, "contractimpl");
        for item in &item_impl.items {
            if let syn::ImplItem::Fn(function) = item {
                self.program
                    .functions
                    .entry(function.sig.ident.to_string())
                    .or_default()
                    .push(VisibilityFunction {
                        block: &function.block,
                        parameters: function_parameters(&function.sig),
                    });

                if is_contract_impl
                    && matches!(function.vis, syn::Visibility::Public(_))
                    && is_helper_shaped(&function.sig.ident.to_string())
                {
                    self.program.entrypoints.push(function);
                }
            }
        }
        visit::visit_item_impl(self, item_impl);
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        self.program
            .functions
            .entry(function.sig.ident.to_string())
            .or_default()
            .push(VisibilityFunction {
                block: &function.block,
                parameters: function_parameters(&function.sig),
            });
        visit::visit_item_fn(self, function);
    }
}

#[derive(Clone, Copy)]
struct VisibilityFlow {
    states: u8,
    return_states: u8,
    break_states: u8,
    continue_states: u8,
    unsafe_mutation: bool,
}

impl VisibilityFlow {
    const UNAUTHENTICATED: u8 = 0b01;
    const AUTHENTICATED: u8 = 0b10;

    fn unauthenticated() -> Self {
        Self {
            states: Self::UNAUTHENTICATED,
            return_states: 0,
            break_states: 0,
            continue_states: 0,
            unsafe_mutation: false,
        }
    }

    fn authenticated(mut self) -> Self {
        if self.states != 0 {
            self.states = Self::AUTHENTICATED;
        }
        self
    }

    fn with_mutation(mut self) -> Self {
        if self.states & Self::UNAUTHENTICATED != 0 {
            self.unsafe_mutation = true;
        }
        self
    }

    fn merge(self, other: Self) -> Self {
        Self {
            states: self.states | other.states,
            return_states: self.return_states | other.return_states,
            break_states: self.break_states | other.break_states,
            continue_states: self.continue_states | other.continue_states,
            unsafe_mutation: self.unsafe_mutation || other.unsafe_mutation,
        }
    }

    fn return_to_caller(mut self) -> Self {
        self.states |= self.return_states;
        self.return_states = 0;
        self
    }

    fn enter_callee(mut self) -> Self {
        self.return_states = 0;
        self.break_states = 0;
        self.continue_states = 0;
        self
    }

    fn restore_caller_exits(mut self, caller: Self) -> Self {
        self.return_states |= caller.return_states;
        self.break_states |= caller.break_states;
        self.continue_states |= caller.continue_states;
        self
    }

    fn enter_loop(mut self) -> (Self, u8, u8) {
        let outer_breaks = self.break_states;
        let outer_continues = self.continue_states;
        self.break_states = 0;
        self.continue_states = 0;
        (self, outer_breaks, outer_continues)
    }

    fn restore_outer_loop(mut self, outer_breaks: u8, outer_continues: u8) -> Self {
        self.break_states |= outer_breaks;
        self.continue_states |= outer_continues;
        self
    }

    fn with_continue(mut self) -> Self {
        self.continue_states |= self.states;
        self.states = 0;
        self
    }

    fn finish_optional_loop(mut self) -> Self {
        self.states |= self.break_states | self.continue_states;
        self.break_states = 0;
        self.continue_states = 0;
        self
    }

    fn finish_mandatory_loop(mut self) -> Self {
        self.states = self.break_states;
        self.break_states = 0;
        self.continue_states = 0;
        self
    }
}

fn function_parameters(signature: &syn::Signature) -> Vec<Option<String>> {
    signature
        .inputs
        .iter()
        .map(|argument| match argument {
            syn::FnArg::Receiver(_) => Some("self".to_string()),
            syn::FnArg::Typed(argument) => pattern_identifier(&argument.pat),
        })
        .collect()
}

fn pattern_identifier(pattern: &syn::Pat) -> Option<String> {
    match pattern {
        syn::Pat::Ident(binding) => Some(binding.ident.to_string()),
        syn::Pat::Paren(pattern) => pattern_identifier(&pattern.pat),
        syn::Pat::Reference(pattern) => pattern_identifier(&pattern.pat),
        syn::Pat::Type(pattern) => pattern_identifier(&pattern.pat),
        _ => None,
    }
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
    })
}

fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        match &attr.meta {
            syn::Meta::List(list) => list
                .tokens
                .to_string()
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .any(|part| part == "test"),
            _ => false,
        }
    })
}

fn is_helper_shaped(name: &str) -> bool {
    name.starts_with('_')
        || name.starts_with("helper_")
        || name.ends_with("_helper")
        || name.starts_with("internal_")
        || name.ends_with("_internal")
}

fn is_auth_call(name: &str) -> bool {
    matches!(name, "require_auth" | "require_auth_for_args")
}

fn is_auth_path(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| is_auth_call(&segment.ident.to_string()))
}

fn path_name(expression: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(path) = expression else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_qualified_storage_mutation(expression: &syn::Expr) -> bool {
    let syn::Expr::Path(path) = expression else {
        return false;
    };
    let Some(last) = path.path.segments.last() else {
        return false;
    };
    if path.path.segments.len() < 2 || !is_mutation_helper_name(&last.ident.to_string()) {
        return false;
    }

    path.path
        .segments
        .iter()
        .take(path.path.segments.len() - 1)
        .any(|segment| segment.ident.to_string().to_lowercase().contains("storage"))
}

fn is_mutation_helper_name(name: &str) -> bool {
    matches!(
        name.split('_').next(),
        Some("write" | "set" | "update" | "remove" | "delete" | "put" | "clear")
    )
}

fn is_storage_mutation_method(name: &str) -> bool {
    matches!(name, "set" | "update" | "try_update" | "remove")
}

fn is_storage_access_method(name: &str) -> bool {
    matches!(
        name,
        "storage"
            | "persistent"
            | "temporary"
            | "instance"
            | "get"
            | "has"
            | "extend_ttl"
            | "max_ttl"
    )
}

fn expr_is_storage_handle(expression: &syn::Expr, aliases: &HashSet<String>) -> bool {
    match expression {
        syn::Expr::MethodCall(call) => {
            let method = call.method.to_string();
            method == "storage"
                || (matches!(method.as_str(), "persistent" | "temporary" | "instance")
                    && expr_is_storage_handle(&call.receiver, aliases))
        }
        syn::Expr::Path(path) => path
            .path
            .get_ident()
            .is_some_and(|identifier| aliases.contains(&identifier.to_string())),
        syn::Expr::Group(group) => expr_is_storage_handle(&group.expr, aliases),
        syn::Expr::Paren(paren) => expr_is_storage_handle(&paren.expr, aliases),
        syn::Expr::Reference(reference) => expr_is_storage_handle(&reference.expr, aliases),
        _ => false,
    }
}

fn check_fn_body(
    block: &syn::Block,
    has_mutation: &mut bool,
    has_read: &mut bool,
    has_auth: &mut bool,
) {
    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Expr(expr, _) => check_expr(expr, has_mutation, has_read, has_auth),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    check_expr(&init.expr, has_mutation, has_read, has_auth);
                }
            }
            syn::Stmt::Macro(m)
                if (m.mac.path.is_ident("require_auth")
                    || m.mac.path.is_ident("require_auth_for_args")) =>
            {
                *has_auth = true;
            }
            _ => {}
        }
    }
}

fn check_expr(expr: &syn::Expr, has_mutation: &mut bool, has_read: &mut bool, has_auth: &mut bool) {
    match expr {
        syn::Expr::Call(c) => {
            if let syn::Expr::Path(p) = &*c.func {
                if let Some(segment) = p.path.segments.last() {
                    let ident = segment.ident.to_string();
                    if ident == "require_auth" || ident == "require_auth_for_args" {
                        *has_auth = true;
                    }
                }
            }
            for arg in &c.args {
                check_expr(arg, has_mutation, has_read, has_auth);
            }
        }
        syn::Expr::MethodCall(m) => {
            let method_name = m.method.to_string();
            if method_name == "set" || method_name == "update" || method_name == "remove" {
                let receiver_str = quote::quote!(#m.receiver).to_string();
                if receiver_str.contains("storage")
                    || receiver_str.contains("persistent")
                    || receiver_str.contains("temporary")
                    || receiver_str.contains("instance")
                {
                    *has_mutation = true;
                }
            }
            if method_name == "get" {
                let receiver_str = quote::quote!(#m.receiver).to_string();
                if receiver_str.contains("storage")
                    || receiver_str.contains("persistent")
                    || receiver_str.contains("temporary")
                    || receiver_str.contains("instance")
                {
                    *has_read = true;
                }
            }
            if method_name == "require_auth" || method_name == "require_auth_for_args" {
                *has_auth = true;
            }
            check_expr(&m.receiver, has_mutation, has_read, has_auth);
            for arg in &m.args {
                check_expr(arg, has_mutation, has_read, has_auth);
            }
        }
        syn::Expr::Block(b) => check_fn_body(&b.block, has_mutation, has_read, has_auth),
        syn::Expr::If(i) => {
            check_expr(&i.cond, has_mutation, has_read, has_auth);
            check_fn_body(&i.then_branch, has_mutation, has_read, has_auth);
            if let Some((_, else_expr)) = &i.else_branch {
                check_expr(else_expr, has_mutation, has_read, has_auth);
            }
        }
        syn::Expr::Match(m) => {
            check_expr(&m.expr, has_mutation, has_read, has_auth);
            for arm in &m.arms {
                check_expr(&arm.body, has_mutation, has_read, has_auth);
            }
        }
        _ => {}
    }
}
