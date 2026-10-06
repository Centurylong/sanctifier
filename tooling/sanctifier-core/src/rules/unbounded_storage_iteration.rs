use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::Attribute;

const FINDING_CODE: &str = "SANCT_UNBOUNDED_STORAGE_ITERATION";

/// Detects loops whose iterator comes from a persistent/instance storage-loaded
/// collection and has no visible bound or pagination step.
pub struct UnboundedStorageIterationRule;

impl UnboundedStorageIterationRule {
    pub fn new() -> Self { Self }
}

impl Default for UnboundedStorageIterationRule {
    fn default() -> Self { Self::new() }
}

impl Rule for UnboundedStorageIterationRule {
    fn name(&self) -> &str { "unbounded_storage_iteration" }

    fn description(&self) -> &str {
        "Detects iteration over persistent/instance storage-loaded collections without a visible bound or pagination step"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };
        let mut visitor = ContractVisitor { violations: Vec::new(), test_depth: 0 };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

struct ContractVisitor {
    violations: Vec<RuleViolation>,
    test_depth: usize,
}

impl ContractVisitor {
    fn in_test_module(&self) -> bool { self.test_depth > 0 }
}

impl<'ast> Visit<'ast> for ContractVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let is_test = has_cfg_test(&node.attrs);
        if is_test { self.test_depth += 1; }
        syn::visit::visit_item_mod(self, node);
        if is_test { self.test_depth -= 1; }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if self.in_test_module() || !has_attr(&node.attrs, "contractimpl") {
            syn::visit::visit_item_impl(self, node);
            return;
        }
        for item in &node.items {
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) { continue; }
                self.violations.extend(check_function(&function.sig.ident.to_string(), &function.block));
            }
        }
    }
}

fn check_function(fn_name: &str, block: &syn::Block) -> Vec<RuleViolation> {
    let mut loads = StorageLoadVisitor::default();
    loads.visit_block(block);
    if loads.collections.is_empty() { return Vec::new(); }

    let mut visitor = IterationVisitor {
        storage_collections: &loads.collections,
        active_caps: HashSet::new(),
        iterations: Vec::new(),
    };
    visitor.visit_block(block);

    visitor.iterations.into_iter()
        .filter(|item| !item.iterator_bounded && !item.context_bounded)
        .map(|item| {
            RuleViolation::new(
                FINDING_CODE,
                Severity::Warning,
                format!(
                    "{FINDING_CODE}: `{fn_name}` iterates storage-loaded collection `{}` without a visible bound or pagination step; worst-case cost is O(n) in stored collection length",
                    item.collection
                ),
                format!("{fn_name}:{}", item.line),
            ).with_suggestion(format!(
                "Check `{0}.len()` against a maximum or process a bounded page (for example with `{0}.iter().take(MAX_PER_CALL)`) before iterating stored entries",
                item.collection
            ))
        }).collect()
}

#[derive(Default)]
struct StorageLoadVisitor { collections: HashSet<String> }

impl<'ast> Visit<'ast> for StorageLoadVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(init) = &node.init {
            if contains_durable_storage_get(&init.expr) {
                if let Some(name) = pat_ident(&node.pat) { self.collections.insert(name); }
            }
        }
        syn::visit::visit_local(self, node);
    }
}

struct IterationVisitor<'a> {
    storage_collections: &'a HashSet<String>,
    active_caps: HashSet<String>,
    iterations: Vec<StorageIteration>,
}

#[derive(Clone)]
struct StorageIteration {
    collection: String,
    line: usize,
    iterator_bounded: bool,
    context_bounded: bool,
}

impl<'ast> Visit<'ast> for IterationVisitor<'_> {
    fn visit_block(&mut self, node: &'ast syn::Block) {
        let saved = self.active_caps.clone();
        syn::visit::visit_block(self, node);
        self.active_caps = saved;
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        let saved = self.active_caps.clone();
        self.active_caps.extend(guaranteed_upper_bounds(
            &node.cond,
            self.storage_collections,
        ));
        self.visit_block(&node.then_branch);
        self.active_caps = saved.clone();

        if let Some((_, else_expr)) = &node.else_branch {
            self.visit_expr(else_expr);
        }
        self.active_caps = saved;
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        let saved = self.active_caps.clone();
        self.active_caps.extend(guaranteed_upper_bounds(
            &node.cond,
            self.storage_collections,
        ));
        self.visit_block(&node.body);
        self.active_caps = saved;
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        self.active_caps.extend(runtime_guard_upper_bounds(
            node,
            self.storage_collections,
        ));
        syn::visit::visit_macro(self, node);
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        if let Some((collection, iterator_bounded)) =
            iterated_storage_collection(&node.expr, self.storage_collections)
        {
            let context_bounded = self.active_caps.contains(&collection);
            self.iterations.push(StorageIteration {
                collection,
                line: node.for_token.span.start().line,
                iterator_bounded,
                context_bounded,
            });
        }
        syn::visit::visit_expr_for_loop(self, node);
    }
}

fn guaranteed_upper_bounds(
    expr: &syn::Expr,
    storage_collections: &HashSet<String>,
) -> HashSet<String> {
    match expr {
        syn::Expr::Paren(paren) => guaranteed_upper_bounds(&paren.expr, storage_collections),
        syn::Expr::Group(group) => guaranteed_upper_bounds(&group.expr, storage_collections),
        syn::Expr::Binary(binary) if matches!(binary.op, syn::BinOp::And(_)) => {
            let mut bounds = guaranteed_upper_bounds(&binary.left, storage_collections);
            bounds.extend(guaranteed_upper_bounds(&binary.right, storage_collections));
            bounds
        }
        syn::Expr::Binary(binary) => {
            let mut bounds = HashSet::new();

            if matches!(
                binary.op,
                syn::BinOp::Lt(_) | syn::BinOp::Le(_) | syn::BinOp::Eq(_)
            ) {
                if let Some(name) = len_call_collection(&binary.left) {
                    if storage_collections.contains(&name) {
                        bounds.insert(name);
                    }
                }
            }

            if matches!(
                binary.op,
                syn::BinOp::Gt(_) | syn::BinOp::Ge(_) | syn::BinOp::Eq(_)
            ) {
                if let Some(name) = len_call_collection(&binary.right) {
                    if storage_collections.contains(&name) {
                        bounds.insert(name);
                    }
                }
            }

            bounds
        }
        _ => HashSet::new(),
    }
}

fn runtime_guard_upper_bounds(
    node: &syn::Macro,
    storage_collections: &HashSet<String>,
) -> HashSet<String> {
    let Some(name) = node.path.segments.last().map(|segment| segment.ident.to_string()) else {
        return HashSet::new();
    };

    if !matches!(name.as_str(), "assert" | "assert_eq" | "ensure" | "require") {
        return HashSet::new();
    }

    let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let Ok(args) = parser.parse2(node.tokens.clone()) else {
        return HashSet::new();
    };
    let mut args = args.iter();
    let Some(first) = args.next() else {
        return HashSet::new();
    };

    if name == "assert_eq" {
        let Some(second) = args.next() else {
            return HashSet::new();
        };
        let mut bounds = HashSet::new();
        if let Some(collection) = len_call_collection(first) {
            if storage_collections.contains(&collection) {
                bounds.insert(collection);
            }
        }
        if let Some(collection) = len_call_collection(second) {
            if storage_collections.contains(&collection) {
                bounds.insert(collection);
            }
        }
        return bounds;
    }

    guaranteed_upper_bounds(first, storage_collections)
}

fn len_call_collection(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Paren(paren) => len_call_collection(&paren.expr),
        syn::Expr::Group(group) => len_call_collection(&group.expr),
        syn::Expr::MethodCall(call) if call.method == "len" && call.args.is_empty() => {
            simple_path_ident(&call.receiver)
        }
        _ => None,
    }
}

fn is_same_collection_len(expr: &syn::Expr, collection: &str) -> bool {
    match expr {
        syn::Expr::Paren(paren) => is_same_collection_len(&paren.expr, collection),
        syn::Expr::Group(group) => is_same_collection_len(&group.expr, collection),
        syn::Expr::MethodCall(call) if call.method == "len" && call.args.is_empty() => {
            simple_path_ident(&call.receiver).as_deref() == Some(collection)
        }
        _ => false,
    }
}

fn iterated_storage_collection(expr: &syn::Expr, storage_collections: &HashSet<String>) -> Option<(String, bool)> {
    match expr {
        syn::Expr::Path(_) => simple_path_ident(expr)
            .filter(|name| storage_collections.contains(name))
            .map(|name| (name, false)),
        syn::Expr::Reference(reference) => iterated_storage_collection(&reference.expr, storage_collections),
        syn::Expr::MethodCall(call) => {
            let (name, bounded) = iterated_storage_collection(&call.receiver, storage_collections)?;
            let effective_take_bound = call.method == "take"
                && call
                    .args
                    .iter()
                    .next()
                    .is_some_and(|limit| !is_same_collection_len(limit, &name));
            Some((name, bounded || effective_take_bound))
        }
        _ => None,
    }
}

fn contains_durable_storage_get(expr: &syn::Expr) -> bool {
    struct DurableGetVisitor { found: bool }
    impl<'ast> Visit<'ast> for DurableGetVisitor {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            if matches!(node.method.to_string().as_str(), "get" | "try_get") && is_durable_storage_chain(&node.receiver) {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }
    let mut visitor = DurableGetVisitor { found: false };
    visitor.visit_expr(expr);
    visitor.found
}

fn is_durable_storage_chain(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(call) if call.method == "persistent" || call.method == "instance" => true,
        syn::Expr::MethodCall(call) => is_durable_storage_chain(&call.receiver),
        _ => false,
    }
}

fn simple_path_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => Some(path.path.segments[0].ident.to_string()),
        syn::Expr::Reference(reference) => simple_path_ident(&reference.expr),
        _ => None,
    }
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        syn::Pat::Type(typed) => pat_ident(&typed.pat),
        _ => None,
    }
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().segments.last().is_some_and(|segment| segment.ident == name))
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") { return false; }
        match &attr.meta {
            syn::Meta::List(list) => list.tokens.to_string()
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
    fn flags_unbounded_and_accepts_bounds() {
        let bad = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        for member in members.iter() { consume(member); }
    }
}"#;
        let good = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        assert!(members.len() <= 100);
        for member in members.iter() { consume(member); }
    }
}"#;
        let paged = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        for member in members.iter().take(100) { consume(member); }
    }
}"#;
        let ineffective_take = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        for member in members.iter().take(members.len()) { consume(member); }
    }
}"#;

        let findings = UnboundedStorageIterationRule::new().check(bad);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(UnboundedStorageIterationRule::new().check(good).is_empty());
        assert!(UnboundedStorageIterationRule::new().check(paged).is_empty());
        assert_eq!(
            UnboundedStorageIterationRule::new()
                .check(ineffective_take)
                .len(),
            1,
            "take(collection.len()) still scans the whole storage collection"
        );

        let incidental_check = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        if members.len() <= 100 { record_metric(); }
        for member in members.iter() { consume(member); }
    }
}"#;
        let non_bound = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        assert!(members.len() != 0);
        debug_assert!(members.len() <= 100);
        for member in members.iter() { consume(member); }
    }
}"#;
        let branch_bound = r#"#[contractimpl]
impl Contract {
    pub fn scan(env: Env) {
        let members: Vec<Address> = env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
        if members.len() <= 100 {
            for member in members.iter() { consume(member); }
        }
    }
}"#;

        assert_eq!(UnboundedStorageIterationRule::new().check(incidental_check).len(), 1);
        assert_eq!(UnboundedStorageIterationRule::new().check(non_bound).len(), 1);
        assert!(UnboundedStorageIterationRule::new().check(branch_bound).is_empty());

        insta::assert_yaml_snapshot!(findings, @r###"
        - rule_name: SANCT_UNBOUNDED_STORAGE_ITERATION
          severity: Warning
          message: "SANCT_UNBOUNDED_STORAGE_ITERATION: `scan` iterates storage-loaded collection `members` without a visible bound or pagination step; worst-case cost is O(n) in stored collection length"
          location: "scan:5"
          suggestion: "Check `members.len()` against a maximum or process a bounded page (for example with `members.iter().take(MAX_PER_CALL)`) before iterating stored entries"
        "###);
    }
}
