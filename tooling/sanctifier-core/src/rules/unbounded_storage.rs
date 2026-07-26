use crate::finding_codes::UNBOUNDED_STORAGE;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Collection-growth methods that append entries without removing any.
const GROWTH_METHODS: &[&str] = &["push_back", "push_front", "push", "append", "insert", "set"];

/// Methods that shrink or bound a collection, signalling the author manages size.
const REMOVAL_METHODS: &[&str] = &[
    "pop_back",
    "pop_front",
    "pop",
    "remove",
    "clear",
    "split_off",
    "truncate",
    "pop_first",
    "pop_last",
    "remove_unbounded",
];

/// Flags Soroban contract entrypoints that grow a persistent/instance storage
/// collection (Vec/Map) with an append/insert but never prune it or cap its
/// length, so the durable entry can grow without bound and eventually brick the
/// contract on ledger-size limits.
pub struct UnboundedStorageRule;

impl UnboundedStorageRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnboundedStorageRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UnboundedStorageRule {
    fn name(&self) -> &str {
        "unbounded_storage"
    }

    fn description(&self) -> &str {
        "Detects persistent/instance storage collections grown without a removal or length cap"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractVisitor {
            violations: Vec::new(),
            suppressions: suppressions(source),
            test_depth: 0,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ContractVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ContractVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ContractVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_test = has_cfg_test(&node.attrs);
        if was_test {
            self.test_depth += 1;
        }

        syn::visit::visit_item_mod(self, node);

        if was_test {
            self.test_depth -= 1;
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if self.in_test_module() || !has_attr(&node.attrs, "contractimpl") {
            syn::visit::visit_item_impl(self, node);
            return;
        }

        for item in &node.items {
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) {
                    continue;
                }

                let fn_name = function.sig.ident.to_string();
                self.violations.extend(
                    check_function(&fn_name, &function.block)
                        .into_iter()
                        .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                        .map(Growth::into_violation),
                );
            }
        }
    }
}

/// A collection identifier grown inside a single entrypoint, together with the
/// bookkeeping needed to decide whether that growth is bounded.
#[derive(Default)]
struct FunctionFacts {
    /// ident -> first append/insert (method, line) seen for it.
    grown: Vec<Growth>,
    /// idents whose value is written back to persistent/instance storage.
    persisted: HashSet<String>,
    /// idents that are pruned (pop/remove/clear/...) somewhere in the function.
    pruned: HashSet<String>,
    /// idents whose `.len()` is read (a visible cap check on the collection).
    length_checked: HashSet<String>,
}

#[derive(Clone)]
struct Growth {
    fn_name: String,
    collection: String,
    method: String,
    line: usize,
}

impl Growth {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            UNBOUNDED_STORAGE,
            Severity::Warning,
            format!(
                "{UNBOUNDED_STORAGE}: `{}` grows persistent collection `{}` via `{}` but never prunes entries or caps its length",
                self.fn_name, self.collection, self.method
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(format!(
            "Cap `{0}.len()` before `{1}`, or remove/pop stale entries so the durable `{0}` entry cannot grow without bound",
            self.collection, self.method
        ))
    }
}

fn check_function(fn_name: &str, block: &syn::Block) -> Vec<Growth> {
    let mut facts = FunctionFacts::default();
    let mut visitor = FactVisitor {
        fn_name,
        facts: &mut facts,
    };
    visitor.visit_block(block);

    // Flag a grown collection only when its value is persisted durably and it is
    // neither pruned nor length-capped anywhere in the same entrypoint.
    let mut seen = HashSet::new();
    facts
        .grown
        .iter()
        .filter(|growth| facts.persisted.contains(&growth.collection))
        .filter(|growth| !facts.pruned.contains(&growth.collection))
        .filter(|growth| !facts.length_checked.contains(&growth.collection))
        .filter(|growth| seen.insert(growth.collection.clone()))
        .cloned()
        .collect()
}

struct FactVisitor<'a> {
    fn_name: &'a str,
    facts: &'a mut FunctionFacts,
}

impl<'ast> Visit<'ast> for FactVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if method == "len" {
            if let Some(ident) = simple_path_ident(&node.receiver) {
                self.facts.length_checked.insert(ident);
            }
        } else if REMOVAL_METHODS.contains(&method.as_str()) {
            if let Some(ident) = simple_path_ident(&node.receiver) {
                self.facts.pruned.insert(ident);
            }
        } else if method == "set" && is_durable_storage_chain(&node.receiver) {
            // `env.storage().persistent().set(&key, &value)` — record the value ident.
            if let Some(value) = node.args.iter().nth(1).and_then(simple_path_ident) {
                self.facts.persisted.insert(value);
            }
        } else if GROWTH_METHODS.contains(&method.as_str()) {
            // A growth op on a plain local collection (not a storage chain).
            if let Some(ident) = simple_path_ident(&node.receiver) {
                self.facts.grown.push(Growth {
                    fn_name: self.fn_name.to_string(),
                    collection: ident,
                    method,
                    line: node.method.span().start().line,
                });
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Returns the identifier for `x` / `&x` / `&mut x`, or `None` for anything else
/// (method chains, calls, field access, ...).
fn simple_path_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => {
            Some(path.path.segments[0].ident.to_string())
        }
        syn::Expr::Reference(reference) => simple_path_ident(&reference.expr),
        _ => None,
    }
}

/// True when the receiver resolves through `.persistent()` or `.instance()`, i.e.
/// a durable Soroban storage handle (temporary storage auto-expires and is
/// excluded).
fn is_durable_storage_chain(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(call) if call.method == "persistent" || call.method == "instance" => {
            true
        }
        syn::Expr::MethodCall(call) => is_durable_storage_chain(&call.receiver),
        _ => false,
    }
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
    })
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

fn suppressions(source: &str) -> Vec<usize> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            line.contains("sanctifier:ignore[SANCT_UNBOUNDED_STORAGE]")
                .then_some(index + 1)
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions
        .iter()
        .any(|suppressed_line| *suppressed_line == line || *suppressed_line + 1 == line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_append_only_persistent_vec() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Vec};

            #[contractimpl]
            impl Contract {
                pub fn register(env: Env, who: Address) {
                    let mut members: Vec<Address> =
                        env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
                    members.push_back(who);
                    env.storage().persistent().set(&KEY, &members);
                }
            }
        "#;

        let findings = UnboundedStorageRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, UNBOUNDED_STORAGE);
        assert!(findings[0].location.contains("register"));
        assert!(findings[0].message.contains("members"));
    }

    #[test]
    fn accepts_length_capped_growth() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Vec};

            #[contractimpl]
            impl Contract {
                pub fn register(env: Env, who: Address) {
                    let mut members: Vec<Address> =
                        env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
                    if members.len() >= 100 {
                        panic!("full");
                    }
                    members.push_back(who);
                    env.storage().persistent().set(&KEY, &members);
                }
            }
        "#;

        assert!(UnboundedStorageRule::new().check(source).is_empty());
    }

    #[test]
    fn accepts_pruned_growth() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Vec};

            #[contractimpl]
            impl Contract {
                pub fn rotate(env: Env, who: Address) {
                    let mut queue: Vec<Address> =
                        env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
                    queue.push_back(who);
                    queue.pop_front();
                    env.storage().persistent().set(&KEY, &queue);
                }
            }
        "#;

        assert!(UnboundedStorageRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_local_collection_not_persisted() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Vec};

            #[contractimpl]
            impl Contract {
                pub fn tally(env: Env, who: Address) {
                    let mut scratch: Vec<Address> = Vec::new(&env);
                    scratch.push_back(who);
                    let _ = scratch.len();
                }
            }
        "#;

        assert!(UnboundedStorageRule::new().check(source).is_empty());
    }

    #[test]
    fn skips_non_contractimpl_and_test_modules() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Vec};

            impl Contract {
                pub fn helper(env: Env, who: Address) {
                    let mut members: Vec<Address> =
                        env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
                    members.push_back(who);
                    env.storage().persistent().set(&KEY, &members);
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[contractimpl]
                impl Contract {
                    pub fn test_register(env: Env, who: Address) {
                        let mut members: Vec<Address> =
                            env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
                        members.push_back(who);
                        env.storage().persistent().set(&KEY, &members);
                    }
                }
            }
        "#;

        assert!(UnboundedStorageRule::new().check(source).is_empty());
    }

    #[test]
    fn honors_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env, Vec};

            #[contractimpl]
            impl Contract {
                pub fn register(env: Env, who: Address) {
                    let mut members: Vec<Address> =
                        env.storage().persistent().get(&KEY).unwrap_or(Vec::new(&env));
                    // sanctifier:ignore[SANCT_UNBOUNDED_STORAGE]
                    members.push_back(who);
                    env.storage().persistent().set(&KEY, &members);
                }
            }
        "#;

        assert!(UnboundedStorageRule::new().check(source).is_empty());
    }
}
