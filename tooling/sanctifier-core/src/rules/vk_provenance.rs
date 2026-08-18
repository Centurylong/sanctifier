use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File, FnArg, ImplItemFn, ItemFn, Pat};

/// Detects a ZK verifying key that's accepted from a runtime parameter and
/// stored with no provenance check — the "swapped-key" bypass: if the VK can
/// be silently changed (or was never pinned to begin with), an attacker can
/// supply their own trusted-setup key and forge proofs against it.
///
/// A hardcoded/compile-time-committed VK (or one gated by `require_auth` or a
/// hash-pin check before it's accepted) is the recognized-safe pattern and is
/// not flagged.
pub struct VkProvenanceRule;

impl VkProvenanceRule {
    pub fn new() -> Self {
        Self
    }

    /// Does this identifier look like it names a ZK verifying key? Matches
    /// snake_case (`vk_bytes`), PascalCase (`VerifyingKey`), and camelCase
    /// alike by comparing on a lowercased, underscore-stripped form.
    fn looks_like_vk_name(ident: &str) -> bool {
        let squashed: String = ident.to_lowercase().chars().filter(|c| *c != '_').collect();
        squashed.contains("verifyingkey")
            || squashed.contains("verificationkey")
            || squashed.starts_with("vkey")
            || squashed.starts_with("vk")
            || squashed.ends_with("vk")
    }

    /// Does this type look like a Soroban byte buffer (`Bytes`, `BytesN<N>`)?
    fn looks_like_bytes_type(ty_tokens: &str) -> bool {
        ty_tokens.contains("Bytes")
    }
}

impl Default for VkProvenanceRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for VkProvenanceRule {
    fn name(&self) -> &str {
        "vk_provenance"
    }

    fn description(&self) -> &str {
        "Detects a ZK verifying key accepted at runtime and stored without a provenance check \
         (auth gate or hash pin), risking a swapped-key bypass"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = VkProvenanceVisitor { issues: Vec::new() };
        visitor.visit_file(&file);
        visitor.issues
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct VkProvenanceVisitor {
    issues: Vec<RuleViolation>,
}

impl VkProvenanceVisitor {
    /// Shared logic for both free functions and impl methods: find
    /// VK-shaped `Bytes` parameters, then check whether the body stores one
    /// into contract storage without a guard.
    fn check_fn(
        &mut self,
        fn_name: &str,
        inputs: &syn::punctuated::Punctuated<FnArg, syn::Token![,]>,
        body: &syn::Block,
        line: usize,
    ) {
        let vk_params: Vec<String> = inputs
            .iter()
            .filter_map(|arg| {
                let FnArg::Typed(pat_type) = arg else {
                    return None;
                };
                let Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
                    return None;
                };
                let name = pat_ident.ident.to_string();
                // `quote!(#pat_type)` renders `NAME : TYPE`; a plain substring
                // check on that is enough to notice a `Bytes`/`BytesN<N>` type
                // without needing to interpolate `pat_type.ty` on its own
                // (quote!'s `#var` interpolation only takes a single token —
                // `#pat_type.ty` would append a literal `.ty` after the whole
                // node's tokens rather than project the field).
                let param_tokens = quote::quote!(#pat_type).to_string();
                if VkProvenanceRule::looks_like_vk_name(&name)
                    && VkProvenanceRule::looks_like_bytes_type(&param_tokens)
                {
                    Some(name)
                } else {
                    None
                }
            })
            .collect();

        if vk_params.is_empty() {
            return;
        }

        let mut store_finder = VkStoreFinder {
            vk_params: &vk_params,
            found_unguarded_store: false,
        };
        store_finder.visit_block(body);

        if !store_finder.found_unguarded_store {
            return;
        }

        let body_tokens = quote::quote!(#body).to_string();
        let has_auth_gate = body_tokens.contains("require_auth");
        let has_hash_pin = body_tokens.to_lowercase().contains("hash")
            && (body_tokens.contains("assert_eq")
                || body_tokens.contains("assert_ne")
                || body_tokens.contains("!=")
                || body_tokens.contains("== "));

        if has_auth_gate || has_hash_pin {
            return;
        }

        self.issues.push(
            RuleViolation::new(
                "vk_provenance",
                Severity::Error,
                format!(
                    "'{fn_name}' stores a verifying key supplied via the '{}' parameter with no \
                     provenance check — any caller can set an arbitrary VK and forge proofs \
                     that verify against it.",
                    vk_params.join("', '")
                ),
                format!("{}:{}", fn_name, line),
            )
            .with_suggestion(
                "Gate the call with require_auth() from a fixed admin, check the incoming key's \
                 hash against a committed constant, or hardcode the verifying key at compile \
                 time instead of accepting it as a parameter."
                    .to_string(),
            ),
        );
    }
}

impl<'ast> Visit<'ast> for VkProvenanceVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.check_fn(
            &node.sig.ident.to_string(),
            &node.sig.inputs,
            &node.block,
            node.span().start().line,
        );
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.check_fn(
            &node.sig.ident.to_string(),
            &node.sig.inputs,
            &node.block,
            node.span().start().line,
        );
        syn::visit::visit_impl_item_fn(self, node);
    }
}

/// Looks for `<recv>.set(<key>, <value>)` (Soroban storage) where `<recv>`'s
/// chain mentions `storage`, `<key>` looks VK-shaped, and `<value>`
/// references one of the function's VK-shaped parameters.
struct VkStoreFinder<'a> {
    vk_params: &'a [String],
    found_unguarded_store: bool,
}

impl<'ast> Visit<'ast> for VkStoreFinder<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "set" && node.args.len() == 2 {
            let receiver = &node.receiver;
            let receiver_tokens = quote::quote!(#receiver).to_string();
            let key_expr = &node.args[0];
            let key_tokens = quote::quote!(#key_expr).to_string();
            let value_expr = &node.args[1];
            let value_tokens = quote::quote!(#value_expr).to_string();

            let receiver_is_storage = receiver_tokens.contains("storage");
            let key_is_vk_shaped = VkProvenanceRule::looks_like_vk_name(&key_tokens);
            let value_references_vk_param = self.vk_params.iter().any(|p| {
                value_tokens
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .any(|tok| tok == p)
            });

            if receiver_is_storage && key_is_vk_shaped && value_references_vk_param {
                self.found_unguarded_store = true;
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_vk_accepted_and_stored_with_no_guard() {
        let rule = VkProvenanceRule::new();
        let source = r#"
            #[contractimpl]
            impl ZkVerifierContract {
                pub fn init(env: Env, vk_bytes: Bytes) {
                    env.storage().instance().set(&DataKey::VerifyingKey, &vk_bytes);
                }
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, Severity::Error);
        assert!(violations[0].message.contains("vk_bytes"));
    }

    #[test]
    fn recognizes_require_auth_gate_as_pinned() {
        let rule = VkProvenanceRule::new();
        let source = r#"
            #[contractimpl]
            impl ZkVerifierContract {
                pub fn init(env: Env, admin: Address, vk_bytes: Bytes) {
                    admin.require_auth();
                    env.storage().instance().set(&DataKey::VerifyingKey, &vk_bytes);
                }
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn recognizes_hash_pin_check_as_committed() {
        let rule = VkProvenanceRule::new();
        let source = r#"
            #[contractimpl]
            impl ZkVerifierContract {
                pub fn init(env: Env, vk_bytes: Bytes) {
                    let digest = hash_bytes(&env, &vk_bytes);
                    assert_eq!(digest, COMMITTED_VK_HASH);
                    env.storage().instance().set(&DataKey::VerifyingKey, &vk_bytes);
                }
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn ignores_hardcoded_compile_time_verifying_key() {
        let rule = VkProvenanceRule::new();
        let source = r#"
            const VK_BYTES: [u8; 4] = [1, 2, 3, 4];

            #[contractimpl]
            impl ZkVerifierContract {
                pub fn init(env: Env) {
                    let vk = Bytes::from_array(&env, &VK_BYTES);
                    env.storage().instance().set(&DataKey::VerifyingKey, &vk);
                }
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn ignores_functions_with_no_vk_shaped_parameter() {
        let rule = VkProvenanceRule::new();
        let source = r#"
            #[contractimpl]
            impl Token {
                pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
                    env.storage().instance().set(&DataKey::Balance(from), &amount);
                }
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn ignores_non_storage_use_of_a_vk_parameter() {
        let rule = VkProvenanceRule::new();
        let source = r#"
            #[contractimpl]
            impl ZkVerifierContract {
                pub fn verify(env: Env, vk_bytes: Bytes, proof_bytes: Bytes) -> bool {
                    let cache: Vec<Bytes> = Vec::new(&env);
                    cache.push_back(vk_bytes.clone());
                    true
                }
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    /// Grounds the detector against the actual contract that motivated it:
    /// `contracts/zk-verifier` accepts its VK via `init(env, vk_bytes: Bytes)`
    /// and stores it under `DataKey::VerifyingKey` with no `require_auth()`
    /// or hash check — this must be flagged.
    #[test]
    fn flags_the_repos_own_zk_verifier_contract() {
        let rule = VkProvenanceRule::new();
        let source = include_str!("../../../../contracts/zk-verifier/src/lib.rs");
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].location, "init:23");
    }
}
