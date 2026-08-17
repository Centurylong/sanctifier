use crate::finding_codes::NULLIFIER_GROWTH;
use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Entrypoint-name fragments that mark a ZK note/nullifier being consumed:
/// spending a note, verifying a proof, or nullifying a leaf/commitment.
const NULLIFIER_FN_HINTS: &[&str] = &[
    "spend",
    "nullify",
    "nullifier",
    "redeem",
    "withdraw",
    "claim",
    "verify",
];

/// Storage-key fragments that mark a per-item nullifier/commitment key, as
/// opposed to an unrelated persistent write in the same entrypoint.
const NULLIFIER_KEY_HINTS: &[&str] = &["nullifier", "commitment", "null_hash", "nullhash", "nf_"];

/// Flags Soroban ZK-verifier contracts that mark a nullifier/commitment as
/// spent by writing it into persistent storage inside a spend/verify/nullify
/// entrypoint, but never prune the entry, extend its TTL, or gate growth with
/// a visible bounded-size check. Every proof spent adds a durable entry that
/// is never reclaimed, so the nullifier set grows without bound and
/// eventually exceeds the ledger size limit.
///
/// This complements `unbounded_storage`: that rule flags a single Vec/Map
/// collection grown via push/insert and written back as a whole; this rule
/// flags the keyed-entry pattern used for nullifier sets (one persistent key
/// per spent note), which `unbounded_storage` cannot see because no local
/// collection is ever grown.
pub struct NullifierGrowthRule;

impl NullifierGrowthRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullifierGrowthRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for NullifierGrowthRule {
    fn name(&self) -> &str {
        "nullifier_growth"
    }

    fn description(&self) -> &str {
        "Detects nullifier/commitment sets grown in persistent storage with no pruning, TTL extension, or bounded-size check"
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

/// A nullifier/commitment key marked spent inside a single entrypoint,
/// together with the bookkeeping needed to decide whether that growth is
/// bounded.
#[derive(Default)]
struct FunctionFacts {
    /// Nullifier-shaped keys written to durable storage via `.set(...)`.
    grown: Vec<Growth>,
    /// True once the function removes a durable entry anywhere (pruning).
    pruned: bool,
    /// True once the function extends/bumps a durable entry's TTL anywhere.
    ttl_extended: bool,
    /// True once the function reads a `.len()` anywhere (a visible cap check).
    length_checked: bool,
}

#[derive(Clone)]
struct Growth {
    fn_name: String,
    key: String,
    line: usize,
}

impl Growth {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            NULLIFIER_GROWTH,
            Severity::Warning,
            format!(
                "{NULLIFIER_GROWTH}: `{}` marks nullifier `{}` as spent in persistent storage but never prunes it, extends its TTL, or caps the nullifier set's size",
                self.fn_name, self.key
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(format!(
            "Extend `{0}`'s TTL with `extend_ttl`, prune stale nullifiers, or cap the nullifier set size so the durable nullifier store cannot grow without bound",
            self.key
        ))
    }
}

fn check_function(fn_name: &str, block: &syn::Block) -> Vec<Growth> {
    if !fn_name_matches_hint(fn_name) {
        return Vec::new();
    }

    let mut facts = FunctionFacts::default();
    let mut visitor = FactVisitor {
        fn_name,
        facts: &mut facts,
    };
    visitor.visit_block(block);

    // Only flag when nothing in the same entrypoint bounds the growth: no
    // pruning, no TTL/expiry extension, and no visible length cap.
    if facts.pruned || facts.ttl_extended || facts.length_checked {
        return Vec::new();
    }

    let mut seen = HashSet::new();
    facts
        .grown
        .into_iter()
        .filter(|growth| seen.insert(growth.key.clone()))
        .collect()
}

struct FactVisitor<'a> {
    fn_name: &'a str,
    facts: &'a mut FunctionFacts,
}

impl<'ast> Visit<'ast> for FactVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if is_durable_storage_chain(&node.receiver) {
            if method == "set" {
                // `env.storage().persistent().set(&nullifier_key, &true)` — a
                // per-item durable write marking a nullifier/commitment spent.
                if let Some(key_expr) = node.args.first() {
                    let key = display_key(key_expr);
                    if looks_like_nullifier_key(&key) {
                        self.facts.grown.push(Growth {
                            fn_name: self.fn_name.to_string(),
                            key,
                            line: node.method.span().start().line,
                        });
                    }
                }
            } else if method == "remove" {
                self.facts.pruned = true;
            } else if method == "extend_ttl" || method.contains("bump") {
                self.facts.ttl_extended = true;
            }
        } else if method == "len" {
            self.facts.length_checked = true;
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// True when the function's own name reads like a note-spend / proof-verify /
/// nullify entrypoint, the shape this rule targets.
fn fn_name_matches_hint(fn_name: &str) -> bool {
    let lower = fn_name.to_ascii_lowercase();
    NULLIFIER_FN_HINTS.iter().any(|hint| lower.contains(hint))
}

/// True when a storage key's source text reads like a nullifier/commitment
/// identifier, as opposed to an unrelated persistent key.
fn looks_like_nullifier_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    NULLIFIER_KEY_HINTS.iter().any(|hint| lower.contains(hint))
}

/// Renders a storage-key expression (stripping a leading `&`) to source text
/// for the nullifier-shape check and the finding message.
fn display_key(expr: &syn::Expr) -> String {
    let key_expr = match expr {
        syn::Expr::Reference(reference) => reference.expr.as_ref(),
        _ => expr,
    };
    quote::quote!(#key_expr).to_string()
}

/// True when the receiver resolves through `.persistent()` or `.instance()`,
/// i.e. a durable Soroban storage handle (temporary storage auto-expires and
/// is excluded).
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
            line.contains("sanctifier:ignore[SANCT_NULLIFIER_GROWTH]")
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
    fn flags_unpruned_nullifier_spend() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, BytesN, Env};

            #[contractimpl]
            impl ShieldedPool {
                pub fn spend_note(env: Env, nullifier: BytesN<32>, proof: BytesN<192>) {
                    verify_proof(&env, &proof);
                    env.storage().persistent().set(&nullifier, &true);
                }
            }
        "#;

        let findings = NullifierGrowthRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, NULLIFIER_GROWTH);
        assert!(findings[0].location.contains("spend_note"));
        assert!(findings[0].message.contains("nullifier"));
    }

    #[test]
    fn accepts_ttl_extended_nullifier() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, BytesN, Env};

            #[contractimpl]
            impl ShieldedPool {
                pub fn spend_note(env: Env, nullifier: BytesN<32>) {
                    env.storage().persistent().set(&nullifier, &true);
                    env.storage().persistent().extend_ttl(&nullifier, 100, 1000);
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }

    #[test]
    fn accepts_pruned_nullifier() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, BytesN, Env};

            #[contractimpl]
            impl ShieldedPool {
                pub fn nullify_expired(env: Env, nullifier: BytesN<32>, stale: BytesN<32>) {
                    env.storage().persistent().set(&nullifier, &true);
                    env.storage().persistent().remove(&stale);
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }

    #[test]
    fn accepts_length_capped_nullifier_set() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, BytesN, Env, Vec};

            #[contractimpl]
            impl ShieldedPool {
                pub fn spend_note(env: Env, nullifier: BytesN<32>) {
                    let mut set: Vec<BytesN<32>> = Vec::new(&env);
                    if set.len() < 1_000_000 {
                        env.storage().persistent().set(&nullifier, &true);
                    }
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_unrelated_persistent_write_in_spend_function() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Vault {
                pub fn withdraw(env: Env, who: Address, amount: i128) {
                    env.storage().persistent().set(&who, &amount);
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }

    #[test]
    fn ignores_nullifier_write_outside_hinted_function() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, BytesN, Env};

            #[contractimpl]
            impl ShieldedPool {
                pub fn record(env: Env, nullifier: BytesN<32>) {
                    env.storage().persistent().set(&nullifier, &true);
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }

    #[test]
    fn skips_non_contractimpl_and_test_modules() {
        let source = r#"
            use soroban_sdk::{contractimpl, BytesN, Env};

            impl ShieldedPool {
                pub fn spend_note(env: Env, nullifier: BytesN<32>) {
                    env.storage().persistent().set(&nullifier, &true);
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[contractimpl]
                impl ShieldedPool {
                    pub fn test_spend_note(env: Env, nullifier: BytesN<32>) {
                        env.storage().persistent().set(&nullifier, &true);
                    }
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }

    #[test]
    fn honors_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, BytesN, Env};

            #[contractimpl]
            impl ShieldedPool {
                pub fn spend_note(env: Env, nullifier: BytesN<32>) {
                    // sanctifier:ignore[SANCT_NULLIFIER_GROWTH]
                    env.storage().persistent().set(&nullifier, &true);
                }
            }
        "#;

        assert!(NullifierGrowthRule::new().check(source).is_empty());
    }
}
