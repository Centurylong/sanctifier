use crate::finding_codes::STORAGE_READ_IN_LOOP;
use crate::rules::{Rule, RuleViolation, Severity};
use quote::ToTokens;
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Expr, File, Pat};

/// Detects loop-invariant Soroban storage reads that can be hoisted out of a
/// loop. The rule is deliberately conservative: it ignores reads whose key
/// depends on a loop variable or a value mutated by the loop, and it suppresses
/// the loop entirely when the loop body mutates storage.
pub struct StorageReadInLoopRule;

impl StorageReadInLoopRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StorageReadInLoopRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for StorageReadInLoopRule {
    fn name(&self) -> &str {
        "storage_read_in_loop"
    }

    fn description(&self) -> &str {
        "Detects loop-invariant Soroban storage reads that can be hoisted outside the loop"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = LoopVisitor {
            current_fn: None,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct LoopVisitor {
    current_fn: Option<String>,
    violations: Vec<RuleViolation>,
}

impl LoopVisitor {
    fn inspect_loop(&mut self, body: &syn::Block, mut variants: HashSet<String>) {
        let mut mutation_facts = MutationVisitor {
            variants: HashSet::new(),
            mutates_storage: false,
        };
        mutation_facts.visit_block(body);
        variants.extend(mutation_facts.variants);

        // Hoisting a read is not semantics-preserving if the loop writes storage.
        // Be conservative and suppress the entire loop rather than trying to
        // prove per-key aliasing from syntax alone.
        if mutation_facts.mutates_storage {
            return;
        }

        let mut reads = ReadVisitor {
            variants: &variants,
            reads: Vec::new(),
        };
        reads.visit_block(body);

        let fn_name = self.current_fn.as_deref().unwrap_or("<function>");
        for read in reads.reads {
            self.violations.push(
                RuleViolation::new(
                    STORAGE_READ_IN_LOOP,
                    Severity::Warning,
                    format!(
                        "{STORAGE_READ_IN_LOOP}: `{fn_name}` performs loop-invariant storage read `{}` inside a loop",
                        read.rendered
                    ),
                    format!("{fn_name}:{}", read.line),
                )
                .with_suggestion(
                    "Hoist this invariant storage read before the loop and reuse the loaded value inside the loop"
                        .to_string(),
                ),
            );
        }
    }
}

impl<'ast> Visit<'ast> for LoopVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let previous = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = previous;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.current_fn = previous;
    }

    fn visit_expr_for_loop(&mut self, node: &'ast syn::ExprForLoop) {
        let mut variants = HashSet::new();
        collect_pat_idents(&node.pat, &mut variants);
        self.inspect_loop(&node.body, variants);

        // Continue normal traversal so nested loops are analysed independently.
        syn::visit::visit_expr(self, &node.expr);
        syn::visit::visit_block(self, &node.body);
    }

    fn visit_expr_while(&mut self, node: &'ast syn::ExprWhile) {
        self.inspect_loop(&node.body, HashSet::new());
        syn::visit::visit_expr(self, &node.cond);
        syn::visit::visit_block(self, &node.body);
    }

    fn visit_expr_loop(&mut self, node: &'ast syn::ExprLoop) {
        self.inspect_loop(&node.body, HashSet::new());
        syn::visit::visit_block(self, &node.body);
    }
}

struct MutationVisitor {
    variants: HashSet<String>,
    mutates_storage: bool,
}

impl<'ast> Visit<'ast> for MutationVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        collect_pat_idents(&node.pat, &mut self.variants);
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        collect_expr_idents(&node.left, &mut self.variants);
        syn::visit::visit_expr_assign(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(
            node.op,
            syn::BinOp::AddAssign(_)
                | syn::BinOp::SubAssign(_)
                | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_)
                | syn::BinOp::RemAssign(_)
                | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_)
                | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_)
                | syn::BinOp::ShrAssign(_)
        ) {
            collect_expr_idents(&node.left, &mut self.variants);
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if storage_chain(&node.receiver)
            && matches!(
                node.method.to_string().as_str(),
                "set" | "remove" | "update" | "extend_ttl"
            )
        {
            self.mutates_storage = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    // Nested loops get their own analysis. Do not let their mutations suppress
    // an otherwise-hoistable read in the enclosing loop.
    fn visit_expr_for_loop(&mut self, _node: &'ast syn::ExprForLoop) {}
    fn visit_expr_while(&mut self, _node: &'ast syn::ExprWhile) {}
    fn visit_expr_loop(&mut self, _node: &'ast syn::ExprLoop) {}
}

struct StorageRead {
    rendered: String,
    line: usize,
}

struct ReadVisitor<'a> {
    variants: &'a HashSet<String>,
    reads: Vec<StorageRead>,
}

impl<'ast> Visit<'ast> for ReadVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if (method == "get" || method == "has") && storage_chain(&node.receiver) {
            if let Some(key) = node.args.first() {
                let mut key_idents = HashSet::new();
                collect_expr_idents(key, &mut key_idents);
                if key_idents.is_disjoint(self.variants) {
                    self.reads.push(StorageRead {
                        rendered: node.to_token_stream().to_string(),
                        line: node.method.span().start().line,
                    });
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_for_loop(&mut self, _node: &'ast syn::ExprForLoop) {}
    fn visit_expr_while(&mut self, _node: &'ast syn::ExprWhile) {}
    fn visit_expr_loop(&mut self, _node: &'ast syn::ExprLoop) {}
}

fn storage_chain(expr: &Expr) -> bool {
    match expr {
        Expr::MethodCall(call)
            if call.method == "persistent"
                || call.method == "instance"
                || call.method == "temporary" =>
        {
            true
        }
        Expr::MethodCall(call) => storage_chain(&call.receiver),
        _ => false,
    }
}

fn collect_pat_idents(pat: &Pat, out: &mut HashSet<String>) {
    match pat {
        Pat::Ident(ident) => {
            out.insert(ident.ident.to_string());
            if let Some((_at, subpat)) = &ident.subpat {
                collect_pat_idents(subpat, out);
            }
        }
        Pat::Tuple(tuple) => {
            for item in &tuple.elems {
                collect_pat_idents(item, out);
            }
        }
        Pat::TupleStruct(tuple) => {
            for item in &tuple.elems {
                collect_pat_idents(item, out);
            }
        }
        Pat::Struct(strct) => {
            for field in &strct.fields {
                collect_pat_idents(&field.pat, out);
            }
        }
        Pat::Reference(reference) => collect_pat_idents(&reference.pat, out),
        Pat::Slice(slice) => {
            for item in &slice.elems {
                collect_pat_idents(item, out);
            }
        }
        Pat::Type(typed) => collect_pat_idents(&typed.pat, out),
        _ => {}
    }
}

fn collect_expr_idents(expr: &Expr, out: &mut HashSet<String>) {
    struct IdentVisitor<'a>(&'a mut HashSet<String>);
    impl<'ast> Visit<'ast> for IdentVisitor<'_> {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if node.qself.is_none() {
                if let Some(ident) = node.path.get_ident() {
                    self.0.insert(ident.to_string());
                }
            }
            syn::visit::visit_expr_path(self, node);
        }
    }
    IdentVisitor(out).visit_expr(expr);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_invariant_storage_read_in_for_loop() {
        let source = r#"
            impl Contract {
                pub fn scan(env: Env, users: Vec<Address>, config_key: DataKey) {
                    for user in users.iter() {
                        let fee: i128 = env.storage().persistent().get(&config_key).unwrap_or(0);
                        charge(&user, fee);
                    }
                }
            }
        "#;

        let findings = StorageReadInLoopRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, STORAGE_READ_IN_LOOP);
        assert!(findings[0].suggestion.as_deref().unwrap().contains("Hoist"));

        let snapshot = (
            findings[0].rule_name.as_str(),
            findings[0].severity,
            findings[0].suggestion.as_deref(),
        );
        insta::assert_debug_snapshot!(snapshot, @r###"
        (
            "SANCT_STORAGE_READ_IN_LOOP",
            Warning,
            Some(
                "Hoist this invariant storage read before the loop and reuse the loaded value inside the loop",
            ),
        )
        "###);
    }

    #[test]
    fn ignores_loop_dependent_key() {
        let source = r#"
            impl Contract {
                pub fn scan(env: Env, users: Vec<Address>) {
                    for user in users.iter() {
                        let balance: i128 = env.storage().persistent().get(&user).unwrap_or(0);
                        consume(balance);
                    }
                }
            }
        "#;

        assert!(StorageReadInLoopRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_key_derived_from_loop_local_binding() {
        let source = r#"
            impl Contract {
                pub fn scan(env: Env, users: Vec<Address>) {
                    for user in users.iter() {
                        let key = user.clone();
                        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
                        consume(balance);
                    }
                }
            }
        "#;

        assert!(StorageReadInLoopRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_key_mutated_by_while_loop() {
        let source = r#"
            impl Contract {
                pub fn scan(env: Env, mut index: u32) {
                    while index < 10 {
                        let value: i128 = env.storage().persistent().get(&index).unwrap_or(0);
                        index += 1;
                        consume(value);
                    }
                }
            }
        "#;

        assert!(StorageReadInLoopRule::new().check(source).is_empty());
    }

    #[test]
    fn suppresses_when_loop_mutates_storage() {
        let source = r#"
            impl Contract {
                pub fn scan(env: Env, users: Vec<Address>, config_key: DataKey) {
                    for user in users.iter() {
                        let fee: i128 = env.storage().persistent().get(&config_key).unwrap_or(0);
                        env.storage().persistent().set(&user, &fee);
                    }
                }
            }
        "#;

        assert!(StorageReadInLoopRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_storage_read_outside_loop() {
        let source = r#"
            impl Contract {
                pub fn read_once(env: Env, key: DataKey) {
                    let value: i128 = env.storage().persistent().get(&key).unwrap_or(0);
                    consume(value);
                }
            }
        "#;

        assert!(StorageReadInLoopRule::new().check(source).is_empty());
    }
}
