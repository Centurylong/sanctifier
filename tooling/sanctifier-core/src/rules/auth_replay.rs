use crate::finding_codes::AUTH_REPLAY;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;

const FINDING_CODE: &str = AUTH_REPLAY;

/// Detects custom-account `__check_auth` implementations that verify a
/// signature but never consume a nonce or check an expiry.
///
/// A custom account's `__check_auth` is the entire authentication story for
/// that account — unlike `Address::require_auth`, there is no protocol-level
/// replay protection underneath it. A signed payload that is only checked
/// against the public key, with nothing that changes state or that binds the
/// signature to a point in time, remains valid forever and can be replayed by
/// anyone who observes it once.
///
/// The rule is intentionally a coarse but sound heuristic rather than a data-
/// flow proof: it looks for *any* identifier inside the function body that
/// reads as a nonce (read, compare, increment, or store) or an expiry/
/// timestamp check. That is enough to distinguish the vulnerable shape in
/// `sig_payload`-only verification from real implementations, which always
/// name the field they're checking, without requiring the rule to understand
/// what storage layout a given account uses.
pub struct AuthReplayRule;

impl AuthReplayRule {
    pub fn new() -> Self {
        Self
    }

    fn check_fn(&self, name: &str, sig: &syn::Signature, block: &syn::Block) -> Vec<RuleViolation> {
        if name != "__check_auth" {
            return Vec::new();
        }

        let mut collector = ReplayGuardCollector { found: false };
        collector.visit_block(block);
        if collector.found {
            return Vec::new();
        }

        vec![RuleViolation::new(
            FINDING_CODE,
            Severity::Error,
            format!(
                "{FINDING_CODE}: `__check_auth` verifies a signature but never reads/increments a \
                 nonce or checks an expiry — a captured signature can be replayed indefinitely"
            ),
            format!("__check_auth:{}", sig.span().start().line),
        )
        .with_suggestion(
            "Consume a per-signer nonce (read the stored value, require it match the payload, \
             then increment and store it) or bind the signed payload to an expiry/ledger sequence \
             checked against `env.ledger().sequence()` / `env.ledger().timestamp()`."
                .to_string(),
        )]
    }
}

impl Default for AuthReplayRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for AuthReplayRule {
    fn name(&self) -> &str {
        "auth_replay"
    }

    fn description(&self) -> &str {
        "Detects custom-account __check_auth implementations without a nonce or expiry check, allowing signature replay."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(f) => (*f).clone(),
            None => return vec![],
        };

        let mut visitor = FnVisitor {
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

struct FnVisitor<'a> {
    rule: &'a AuthReplayRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FnVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(self.rule.check_fn(
            &node.sig.ident.to_string(),
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_fn(
            &node.sig.ident.to_string(),
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_impl_item_fn(self, node);
    }
}

/// True once any identifier in the visited subtree reads as a nonce or an
/// expiry/timestamp check.
struct ReplayGuardCollector {
    found: bool,
}

impl<'ast> Visit<'ast> for ReplayGuardCollector {
    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        let lower = node.to_string().to_lowercase();
        if is_replay_guard_word(&lower) {
            self.found = true;
        }
    }

    // Storage keys are frequently a string/symbol literal (`symbol_short!("nonce")`,
    // `Symbol::new(&env, "nonce")`) rather than an identifier, so those need
    // checking too.
    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        if is_replay_guard_word(&node.value().to_lowercase()) {
            self.found = true;
        }
    }
}

fn is_replay_guard_word(lower: &str) -> bool {
    lower.contains("nonce")
        || lower.contains("expir")
        || lower.contains("deadline")
        || lower.contains("timestamp")
        || lower.contains("sequence")
}
