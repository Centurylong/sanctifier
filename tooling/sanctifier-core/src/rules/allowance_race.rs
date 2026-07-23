use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_ALLOWANCE_RACE";

/// Detects the classic "approve front-running" race: an allowance write that
/// **overwrites the stored value unconditionally** from a caller-supplied
/// amount, with no `increase_allowance`/`decrease_allowance` (delta) semantics
/// and no compare-and-set (expected-current-value) guard.
///
/// ## Why it matters
/// If `approve(spender, N)` blindly replaces the allowance, a spender who is
/// watching the mempool can front-run a change from `N` to `M`: they spend the
/// old `N`, then the new `M` lands, letting them spend `N + M` in total.
///
/// ## Vulnerable pattern
/// ```ignore
/// pub fn approve(e: Env, owner: Address, spender: Address, amount: i128) {
///     e.storage().persistent().set(&(owner, spender), &amount); // racy
/// }
/// ```
///
/// ## Safe alternatives (not flagged)
/// * **Delta semantics** — a function named `increase_allowance` /
///   `decrease_allowance`, or one whose written value is *computed* from the
///   current allowance (`current + delta`) rather than a bare argument.
/// * **Compare-and-set** — the function reads the current allowance first (a
///   `get` on storage) or takes an `expected_current` / `old` / `prev`
///   parameter it checks before writing.
///
/// ## What marks a write as an allowance write
/// Either the enclosing function is an approval entrypoint (its name contains
/// `approve` or `allowance`) or the storage key itself names an allowance
/// (`K::Allowance(..)`). This catches the canonical `approve(owner, spender,
/// amount)` shape whose key is a bare `(owner, spender)` tuple, while leaving
/// unrelated `set`s (balances, config, …) untouched.
pub struct AllowanceRaceRule;

impl AllowanceRaceRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        fn_name: &str,
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        let mut visitor = AllowanceVisitor::default();
        visitor.visit_block(block);

        let name_lc = fn_name.to_lowercase();
        let fn_is_allowance = name_lc.contains("approve") || name_lc.contains("allowance");

        // A read of the current value before writing (a storage `get`) is the
        // tell-tale of read-modify-write: both delta and compare-and-set
        // implementations do it, so the write is not an unconditional overwrite.
        let reads_current = visitor.reads_storage;
        let delta_named = name_lc.contains("increase")
            || name_lc.contains("decrease")
            || name_lc.contains("incr")
            || name_lc.contains("decr")
            || name_lc.contains("adjust");
        let has_cas_param = sig.inputs.iter().any(param_is_cas);

        if reads_current || delta_named || has_cas_param {
            return vec![];
        }

        visitor
            .overwrites
            .into_iter()
            .filter(|write| fn_is_allowance || write.key_is_allowance)
            .map(|write| {
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    format!(
                        "SANCT_ALLOWANCE_RACE: `{}` overwrites the allowance at key `{}` \
                         unconditionally from a caller-supplied amount, enabling the approve \
                         front-running race",
                        fn_name, write.key
                    ),
                    format!("{}:{}", fn_name, write.line),
                )
                .with_suggestion(
                    "Use delta semantics (`increase_allowance`/`decrease_allowance`) or a \
                     compare-and-set: read the current allowance and require an `expected_current` \
                     argument to match before writing the new value."
                        .to_string(),
                )
            })
            .collect()
    }
}

impl Default for AllowanceRaceRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for AllowanceRaceRule {
    fn name(&self) -> &str {
        "allowance_race"
    }

    fn description(&self) -> &str {
        "Detects set-allowance writes that overwrite unconditionally, lacking increase/decrease or compare-and-set semantics (approve front-run race)"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = AllowanceRaceRuleVisitor {
            rule: self,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct AllowanceRaceRuleVisitor<'rule> {
    rule: &'rule AllowanceRaceRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for AllowanceRaceRuleVisitor<'_> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_item_fn(self, node);
    }
}

#[derive(Default)]
struct AllowanceVisitor {
    overwrites: Vec<AllowanceWrite>,
    reads_storage: bool,
}

struct AllowanceWrite {
    key: String,
    key_is_allowance: bool,
    line: usize,
}

impl<'ast> Visit<'ast> for AllowanceVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        // Only storage-backed accesses are relevant. A `get` on storage means
        // the function reads current state (read-modify-write / compare-and-set).
        if is_storage_receiver(&node.receiver) {
            if method == "get" {
                self.reads_storage = true;
            }

            // A `.set(&key, &value)` is a candidate overwrite. It is only
            // "unconditional" when the written value is a bare identifier (a
            // caller-supplied amount) rather than a computed expression.
            if method == "set" {
                if let (Some(key_expr), Some(value_expr)) =
                    (node.args.first(), node.args.iter().nth(1))
                {
                    if value_is_bare_amount(value_expr) {
                        self.overwrites.push(AllowanceWrite {
                            key: display_expr(key_expr),
                            key_is_allowance: key_is_allowance(key_expr),
                            line: node.span().start().line,
                        });
                    }
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

/// True when the receiver chain terminates in a storage handle, i.e. a
/// `.persistent()`, `.instance()`, or `.temporary()` call somewhere in the
/// method chain (e.g. `env.storage().persistent()`).
fn is_storage_receiver(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::MethodCall(method_call) => {
            let m = method_call.method.to_string();
            m == "persistent"
                || m == "instance"
                || m == "temporary"
                || is_storage_receiver(&method_call.receiver)
        }
        _ => false,
    }
}

/// True when the stringified key expression explicitly names an allowance
/// entry, e.g. `K::Allowance(owner, spender)` or `DataKey::Allowance(..)`.
fn key_is_allowance(expr: &syn::Expr) -> bool {
    display_expr(expr).to_lowercase().contains("allowance")
}

/// True when the value written is a bare identifier reference (`&amount`) — an
/// unconditional overwrite. Computed values (`&(current + delta)`) are not.
fn value_is_bare_amount(expr: &syn::Expr) -> bool {
    let inner = match expr {
        syn::Expr::Reference(reference) => reference.expr.as_ref(),
        other => other,
    };
    matches!(inner, syn::Expr::Path(_))
}

/// True when a parameter name signals a compare-and-set contract, i.e. the
/// caller must pass the value they expect the allowance to currently hold.
fn param_is_cas(arg: &syn::FnArg) -> bool {
    let syn::FnArg::Typed(pat_type) = arg else {
        return false;
    };
    let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
        return false;
    };
    let name = pat_ident.ident.to_string().to_lowercase();
    name.contains("expected")
        || name.contains("current")
        || name == "old"
        || name.starts_with("old_")
        || name.contains("prev")
}

fn display_expr(expr: &syn::Expr) -> String {
    let key_expr = match expr {
        syn::Expr::Reference(reference) => reference.expr.as_ref(),
        other => other,
    };
    quote::quote!(#key_expr)
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unconditional_approve_overwrite() {
        let source = r#"
            impl Token {
                pub fn approve(e: Env, owner: Address, spender: Address, amount: i128) {
                    e.storage().persistent().set(&K::Allowance(owner, spender), &amount);
                }
            }
        "#;
        let findings = AllowanceRaceRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, "SANCT_ALLOWANCE_RACE");
        assert_eq!(findings[0].severity, Severity::Warning);
        assert!(findings[0].message.contains("approve"));
    }

    #[test]
    fn flags_approve_with_bare_tuple_key() {
        // The canonical gallery shape: the allowance key is a bare
        // `(owner, spender)` tuple with no "allowance" in the name, so the
        // function name is what marks it as an allowance write.
        let source = r#"
            impl Token {
                pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
                    owner.require_auth();
                    env.storage().persistent().set(&(owner.clone(), spender.clone()), &amount);
                }
            }
        "#;
        let findings = AllowanceRaceRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, "SANCT_ALLOWANCE_RACE");
    }

    #[test]
    fn ignores_increase_allowance_delta() {
        let source = r#"
            impl Token {
                pub fn increase_allowance(e: Env, owner: Address, spender: Address, delta: i128) {
                    let current: i128 = e.storage().persistent().get(&K::Allowance(owner.clone(), spender.clone())).unwrap_or(0);
                    e.storage().persistent().set(&K::Allowance(owner, spender), &(current + delta));
                }
            }
        "#;
        let findings = AllowanceRaceRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_compare_and_set_with_expected_param() {
        let source = r#"
            impl Token {
                pub fn approve(e: Env, owner: Address, spender: Address, expected_current: i128, amount: i128) {
                    let current: i128 = e.storage().persistent().get(&K::Allowance(owner.clone(), spender.clone())).unwrap_or(0);
                    if current != expected_current {
                        panic!("allowance changed");
                    }
                    e.storage().persistent().set(&K::Allowance(owner, spender), &amount);
                }
            }
        "#;
        let findings = AllowanceRaceRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_non_allowance_set() {
        let source = r#"
            impl Token {
                pub fn set_balance(e: Env, who: Address, amount: i128) {
                    e.storage().persistent().set(&DataKey::Balance(who), &amount);
                }
            }
        "#;
        let findings = AllowanceRaceRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_decrease_allowance_by_name() {
        let source = r#"
            impl Token {
                pub fn decrease_allowance(e: Env, owner: Address, spender: Address, amount: i128) {
                    e.storage().persistent().set(&K::Allowance(owner, spender), &amount);
                }
            }
        "#;
        // Named as a delta adjustment — treated as safe even though the body is thin.
        let findings = AllowanceRaceRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_non_storage_set() {
        // A `.set` on a plain map (not storage) is not an allowance write.
        let source = r#"
            impl Token {
                pub fn approve(e: Env, owner: Address, spender: Address, amount: i128) {
                    let mut cache = Map::new();
                    cache.set(&(owner, spender), &amount);
                }
            }
        "#;
        let findings = AllowanceRaceRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
