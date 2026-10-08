//! Ordered, path-sensitive checks/effects/interactions (CEI) analysis.
//!
//! The pass is deliberately conservative: it reports only calls that can be
//! identified syntactically as Soroban storage writes or cross-contract calls.
//! It forks paths for if/match, ignores nested closures, and records a single
//! loop iteration together with its zero-iteration alternative. It is not a
//! whole-program alias/CFG proof. Callers must not interpret a clean result as
//! proof of reentrancy safety.

use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Block, Expr, File, ImplItemFn, ItemFn, Local, Stmt};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CeiKind {
    Check,
    Effect,
    Interaction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CeiEvent {
    pub kind: CeiKind,
    pub operation: String,
    pub line: usize,
}

#[derive(Clone, Debug, Default)]
pub struct CeiPath {
    pub events: Vec<CeiEvent>,
    pub loops_present: bool,
    pub terminated: bool,
}

#[derive(Clone, Debug)]
pub struct CeiModel {
    pub function: String,
    pub paths: Vec<CeiPath>,
}

impl CeiModel {
    /// A pair is returned only if the interaction and later write can both
    /// execute on the same syntactic path.
    pub fn interactions_before_effects(&self) -> Vec<(CeiEvent, CeiEvent)> {
        let mut found = Vec::new();
        let mut seen = HashSet::new();
        for path in &self.paths {
            for (index, event) in path.events.iter().enumerate() {
                if event.kind != CeiKind::Interaction {
                    continue;
                }
                for effect in path.events[index + 1..]
                    .iter()
                    .filter(|event| event.kind == CeiKind::Effect)
                {
                    if seen.insert((event.line, effect.line, event.operation.clone())) {
                        found.push((event.clone(), effect.clone()));
                    }
                }
            }
        }
        found
    }

    pub fn effects(&self) -> Vec<&CeiEvent> {
        self.paths
            .iter()
            .flat_map(|p| p.events.iter())
            .filter(|event| event.kind == CeiKind::Effect)
            .collect()
    }

    pub fn interactions(&self) -> Vec<&CeiEvent> {
        self.paths
            .iter()
            .flat_map(|p| p.events.iter())
            .filter(|event| event.kind == CeiKind::Interaction)
            .collect()
    }
}

/// Analyze inherent and free Rust functions. Unparseable source yields no
/// models, matching the standard Sanctifier rule parser contract.
pub fn models(source: &str) -> Vec<CeiModel> {
    let file = match syn::parse_str::<File>(source) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let mut visitor = ModelCollector { models: Vec::new() };
    visitor.visit_file(&file);
    visitor.models
}

struct ModelCollector {
    models: Vec<CeiModel>,
}

impl ModelCollector {
    fn collect(&mut self, name: &str, block: &Block) {
        let mut bindings = Bindings::default();
        bindings.visit_block(block);
        let paths = analyze_block(block, vec![CeiPath::default()], &bindings);
        self.models.push(CeiModel {
            function: name.into(),
            paths,
        });
    }
}

impl<'ast> Visit<'ast> for ModelCollector {
    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        self.collect(&function.sig.ident.to_string(), &function.block);
        visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast ImplItemFn) {
        self.collect(&function.sig.ident.to_string(), &function.block);
        visit::visit_impl_item_fn(self, function);
    }
}

/// Recognize locally bound Soroban client and storage handles; do not equate
/// every method named set/transfer with a storage write/cross-contract call.
#[derive(Default)]
struct Bindings {
    clients: HashSet<String>,
    storage: HashSet<String>,
}

impl<'ast> Visit<'ast> for Bindings {
    fn visit_local(&mut self, local: &'ast Local) {
        if let syn::Pat::Ident(pat) = &local.pat {
            if let Some(init) = &local.init {
                let name = pat.ident.to_string();
                if client_constructor(&init.expr) {
                    self.clients.insert(name.clone());
                }
                if storage_handle(&init.expr, self) {
                    self.storage.insert(name);
                }
            }
        }
        visit::visit_local(self, local);
    }
}

fn client_constructor(expr: &Expr) -> bool {
    match expr {
        Expr::Call(call) => match &*call.func {
            Expr::Path(path) => {
                let segments: Vec<String> = path
                    .path
                    .segments
                    .iter()
                    .map(|part| part.ident.to_string())
                    .collect();
                segments.last().is_some_and(|last| last == "new")
                    && segments
                        .iter()
                        .rev()
                        .skip(1)
                        .any(|part| part == "Client" || part.ends_with("Client"))
            }
            _ => false,
        },
        Expr::Reference(e) => client_constructor(&e.expr),
        Expr::Paren(e) => client_constructor(&e.expr),
        Expr::Group(e) => client_constructor(&e.expr),
        _ => false,
    }
}

fn binding_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) if p.path.segments.len() == 1 => Some(p.path.segments[0].ident.to_string()),
        Expr::Reference(e) => binding_ident(&e.expr),
        Expr::Paren(e) => binding_ident(&e.expr),
        Expr::Group(e) => binding_ident(&e.expr),
        _ => None,
    }
}

fn storage_handle(expr: &Expr, bindings: &Bindings) -> bool {
    match expr {
        Expr::MethodCall(call) => {
            matches!(
                call.method.to_string().as_str(),
                "storage" | "persistent" | "temporary" | "instance"
            ) || storage_handle(&call.receiver, bindings)
        }
        Expr::Reference(e) => storage_handle(&e.expr, bindings),
        Expr::Paren(e) => storage_handle(&e.expr, bindings),
        Expr::Group(e) => storage_handle(&e.expr, bindings),
        _ => binding_ident(expr).is_some_and(|name| bindings.storage.contains(&name)),
    }
}

fn client_receiver(expr: &Expr, bindings: &Bindings) -> bool {
    if client_constructor(expr) {
        return true;
    }
    if let Some(name) = binding_ident(expr) {
        // Generated clients are often named *_client, sac or token. This
        // convention also captures arguments typed as a generated client.
        return bindings.clients.contains(&name)
            || name.ends_with("_client")
            || matches!(name.as_str(), "token" | "sac" | "token_client");
    }
    match expr {
        Expr::MethodCall(call) => client_receiver(&call.receiver, bindings),
        Expr::Field(field) => client_receiver(&field.base, bindings),
        _ => false,
    }
}

fn classify_method(node: &syn::ExprMethodCall, bindings: &Bindings) -> Option<CeiEvent> {
    let method = node.method.to_string();
    let kind = if matches!(method.as_str(), "require_auth" | "require_auth_for_args") {
        CeiKind::Check
    } else if matches!(method.as_str(), "set" | "update" | "remove")
        && storage_handle(&node.receiver, bindings)
    {
        CeiKind::Effect
    } else if matches!(method.as_str(), "invoke_contract" | "try_invoke_contract") {
        CeiKind::Interaction
    } else if client_receiver(&node.receiver, bindings)
        && !matches!(
            method.as_str(),
            "new" | "address" | "clone" | "clone_from" | "env" | "contract_address"
        )
    {
        // All generated Client methods issue a host call. In particular SAC
        // transfer/transfer_from, approve, burn and mint are interactions.
        CeiKind::Interaction
    } else {
        return None;
    };
    Some(CeiEvent {
        kind,
        operation: method,
        line: node.method.span().start().line,
    })
}

fn classify_call(node: &syn::ExprCall) -> Option<CeiEvent> {
    let Expr::Path(path) = &*node.func else {
        return None;
    };
    let parts: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|p| p.ident.to_string())
        .collect();
    let last = parts.last()?;
    let lower = last.to_ascii_lowercase();
    // Known storage helper names, not unrelated arbitrary calls.
    if matches!(
        lower.as_str(),
        "write_balance"
            | "save_balance"
            | "store_balance"
            | "write_state"
            | "store_state"
            | "write_storage"
            | "save_storage"
    ) {
        return Some(CeiEvent {
            kind: CeiKind::Effect,
            operation: last.clone(),
            line: node.span().start().line,
        });
    }
    if (last == "transfer" || last == "transfer_from")
        && parts.iter().any(|p| p == "Client" || p.ends_with("Client"))
    {
        return Some(CeiEvent {
            kind: CeiKind::Interaction,
            operation: last.clone(),
            line: node.span().start().line,
        });
    }
    None
}

struct Events<'a> {
    bindings: &'a Bindings,
    events: Vec<CeiEvent>,
}

impl<'ast> Visit<'ast> for Events<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        // Rust evaluates receiver and arguments before the call itself.
        visit::visit_expr_method_call(self, node);
        if let Some(event) = classify_method(node, self.bindings) {
            self.events.push(event);
        }
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        visit::visit_expr_call(self, node);
        if let Some(event) = classify_call(node) {
            self.events.push(event);
        }
    }

    // A nested control-flow expression cannot safely be linearized here.
    // Top-level if/match/loop/block are handled separately by analyze_expr.
    fn visit_expr_if(&mut self, _node: &'ast syn::ExprIf) {}
    fn visit_expr_match(&mut self, _node: &'ast syn::ExprMatch) {}
    fn visit_expr_for_loop(&mut self, _node: &'ast syn::ExprForLoop) {}
    fn visit_expr_while(&mut self, _node: &'ast syn::ExprWhile) {}
    fn visit_expr_loop(&mut self, _node: &'ast syn::ExprLoop) {}
    fn visit_expr_closure(&mut self, _node: &'ast syn::ExprClosure) {}
}

fn inspect_expr(expr: &Expr, paths: Vec<CeiPath>, bindings: &Bindings) -> Vec<CeiPath> {
    match expr {
        Expr::If(node) => {
            let base = inspect_expr(&node.cond, paths, bindings);
            let mut result = analyze_block(&node.then_branch, base.clone(), bindings);
            let alternate = if let Some((_, else_expr)) = &node.else_branch {
                inspect_expr(else_expr, base, bindings)
            } else {
                base
            };
            result.extend(alternate);
            cap(result)
        }
        Expr::Match(node) => {
            let base = inspect_expr(&node.expr, paths, bindings);
            let mut result = Vec::new();
            for arm in &node.arms {
                // Guards are intentionally not assumed to execute all arms.
                result.extend(inspect_expr(&arm.body, base.clone(), bindings));
            }
            cap(result)
        }
        Expr::Block(node) => analyze_block(&node.block, paths, bindings),
        Expr::ForLoop(node) => {
            let base = inspect_expr(&node.expr, paths, bindings);
            once_with_zero_path(&node.body, base, bindings)
        }
        Expr::While(node) => {
            let base = inspect_expr(&node.cond, paths, bindings);
            once_with_zero_path(&node.body, base, bindings)
        }
        Expr::Loop(node) => once_with_zero_path(&node.body, paths, bindings),
        Expr::Return(node) => {
            let result = if let Some(expr) = &node.expr {
                inspect_expr(expr, paths, bindings)
            } else {
                paths
            };
            result
                .into_iter()
                .map(|mut path| {
                    path.terminated = true;
                    path
                })
                .collect()
        }
        Expr::Paren(node) => inspect_expr(&node.expr, paths, bindings),
        Expr::Group(node) => inspect_expr(&node.expr, paths, bindings),
        _ => {
            let mut collector = Events {
                bindings,
                events: Vec::new(),
            };
            collector.visit_expr(expr);
            paths
                .into_iter()
                .map(|mut path| {
                    if !path.terminated {
                        path.events.extend(collector.events.iter().cloned());
                    }
                    path
                })
                .collect()
        }
    }
}

fn once_with_zero_path(block: &Block, paths: Vec<CeiPath>, bindings: &Bindings) -> Vec<CeiPath> {
    let mut no_iteration = paths;
    for path in &mut no_iteration {
        path.loops_present = true;
    }
    let mut one_iteration = analyze_block(block, no_iteration.clone(), bindings);
    for path in &mut one_iteration {
        path.loops_present = true;
        // A break/return inside the loop is not treated as terminating the
        // enclosing function; this loses precision but avoids false alarms.
    }
    no_iteration.extend(one_iteration);
    cap(no_iteration)
}

fn cap(mut paths: Vec<CeiPath>) -> Vec<CeiPath> {
    // Avoid unbounded exponential path growth on generated contract code.
    // Truncation can lose findings, never creates cross-branch false positives.
    paths.truncate(64);
    paths
}

fn analyze_block(block: &Block, mut paths: Vec<CeiPath>, bindings: &Bindings) -> Vec<CeiPath> {
    for stmt in &block.stmts {
        paths = match stmt {
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    inspect_expr(&init.expr, paths, bindings)
                } else {
                    paths
                }
            }
            Stmt::Expr(expr, _) => inspect_expr(expr, paths, bindings),
            Stmt::Item(_) | Stmt::Macro(_) => paths,
        };
    }
    cap(paths)
}
