use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_RESERVE_WITHDRAWAL_AUTH";

/// Detects reserve/treasury withdrawal paths that move pooled funds without a
/// strong admin + nonce authorization guard.
pub struct ReserveWithdrawalAuthRule;

impl ReserveWithdrawalAuthRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        fn_name: &str,
        visibility: &syn::Visibility,
        block: &syn::Block,
        line: usize,
    ) -> Option<RuleViolation> {
        if !matches!(visibility, syn::Visibility::Public(_)) {
            return None;
        }

        let mut visitor = ReserveWithdrawalVisitor::default();
        visitor.visit_block(block);

        let function_name_matches =
            mentions_reserve_domain(fn_name) && mentions_withdrawal(fn_name);
        let body_matches = visitor.mentions_reserve_domain && visitor.moves_funds;

        if !(function_name_matches || body_matches) {
            return None;
        }

        if visitor.has_auth && visitor.has_nonce_guard {
            return None;
        }

        let missing = match (visitor.has_auth, visitor.has_nonce_guard) {
            (false, false) => "admin authorization and nonce/replay guard",
            (false, true) => "admin authorization",
            (true, false) => "nonce/replay guard",
            (true, true) => unreachable!(),
        };

        Some(
            RuleViolation::new(
                FINDING_CODE,
                Severity::Warning,
                format!(
                    "{FINDING_CODE}: reserve/treasury withdrawal path `{fn_name}` is missing {missing}"
                ),
                format!("{fn_name}:{line}"),
            )
            .with_suggestion(
                "Require the admin signer and bind the withdrawal to a consumed nonce before moving reserve funds"
                    .to_string(),
            ),
        )
    }
}

impl Default for ReserveWithdrawalAuthRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ReserveWithdrawalAuthRule {
    fn name(&self) -> &str {
        "reserve_withdrawal_auth"
    }

    fn description(&self) -> &str {
        "Detects reserve/treasury withdrawals missing admin authorization or nonce guards"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = ReserveWithdrawalRuleVisitor {
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

struct ReserveWithdrawalRuleVisitor<'rule> {
    rule: &'rule ReserveWithdrawalAuthRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for ReserveWithdrawalRuleVisitor<'_> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if let Some(violation) = self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.block,
            node.span().start().line,
        ) {
            self.violations.push(violation);
        }
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if let Some(violation) = self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.block,
            node.span().start().line,
        ) {
            self.violations.push(violation);
        }
        syn::visit::visit_item_fn(self, node);
    }
}

#[derive(Default)]
struct ReserveWithdrawalVisitor {
    has_auth: bool,
    has_nonce_guard: bool,
    mentions_reserve_domain: bool,
    moves_funds: bool,
}

impl<'ast> Visit<'ast> for ReserveWithdrawalVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method_name = node.method.to_string();
        let receiver = quote::quote!(#node.receiver).to_string();
        let call = quote::quote!(#node).to_string();

        self.record_tokens(&method_name);
        self.record_tokens(&receiver);
        self.record_tokens(&call);

        if is_auth_method(&method_name) {
            self.has_auth = true;
        }

        if is_fund_movement_method(&method_name) {
            self.moves_funds = true;
        }

        if is_storage_write_method(&method_name)
            && mentions_reserve_domain(&receiver)
            && (call.contains('-') || call.contains("amount"))
        {
            self.moves_funds = true;
        }

        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        let call = quote::quote!(#node).to_string();
        self.record_tokens(&call);

        if let syn::Expr::Path(path) = &*node.func {
            if let Some(segment) = path.path.segments.last() {
                let function_name = segment.ident.to_string();
                if is_auth_method(&function_name) {
                    self.has_auth = true;
                }
                if is_fund_movement_method(&function_name) || mentions_withdrawal(&function_name) {
                    self.moves_funds = true;
                }
            }
        }

        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        let cond = quote::quote!(#node.cond).to_string();
        self.record_tokens(&cond);
        syn::visit::visit_expr_if(self, node);
    }

    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        let matched = quote::quote!(#node.expr).to_string();
        self.record_tokens(&matched);
        syn::visit::visit_expr_match(self, node);
    }

    fn visit_stmt(&mut self, node: &'ast syn::Stmt) {
        let stmt = quote::quote!(#node).to_string();
        self.record_tokens(&stmt);
        syn::visit::visit_stmt(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        let path = quote::quote!(#node.path).to_string();
        let tokens = node.tokens.to_string();

        if is_auth_method(&path) || is_auth_method(&tokens) {
            self.has_auth = true;
        }
        self.record_tokens(&tokens);

        syn::visit::visit_macro(self, node);
    }
}

impl ReserveWithdrawalVisitor {
    fn record_tokens(&mut self, tokens: &str) {
        let normalized = normalize_tokens(tokens);

        if mentions_reserve_domain(&normalized) {
            self.mentions_reserve_domain = true;
        }

        if mentions_nonce_guard(&normalized) {
            self.has_nonce_guard = true;
        }

        if mentions_withdrawal(&normalized) {
            self.moves_funds = true;
        }
    }
}

fn normalize_tokens(tokens: &str) -> String {
    tokens
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

fn mentions_reserve_domain(tokens: &str) -> bool {
    let normalized = normalize_tokens(tokens);
    ["reserve", "treasury", "vault", "pool", "custody"]
        .iter()
        .any(|keyword| normalized.contains(keyword))
}

fn mentions_withdrawal(tokens: &str) -> bool {
    let normalized = normalize_tokens(tokens);
    [
        "withdraw",
        "withdrawal",
        "sweep",
        "drain",
        "release",
        "rescue",
        "payout",
        "transferout",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword))
}

fn mentions_nonce_guard(tokens: &str) -> bool {
    let normalized = normalize_tokens(tokens);
    [
        "nonce",
        "replay",
        "seennonce",
        "consumenonce",
        "recordnonce",
        "incrementnonce",
        "usednonce",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword))
}

fn is_auth_method(method: &str) -> bool {
    matches!(
        normalize_tokens(method).as_str(),
        "requireauth" | "requireauthforargs" | "checkadmin" | "assertadmin" | "ensureadmin"
    )
}

fn is_fund_movement_method(method: &str) -> bool {
    matches!(
        normalize_tokens(method).as_str(),
        "transfer" | "trytransfer" | "send" | "withdraw" | "sweep" | "drain" | "payout"
    )
}

fn is_storage_write_method(method: &str) -> bool {
    matches!(
        normalize_tokens(method).as_str(),
        "set" | "update" | "remove"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_reserve_withdrawal_without_auth_or_nonce() {
        let source = r#"
            use soroban_sdk::{Address, Env};

            impl TreasuryContract {
                pub fn withdraw_reserve(env: Env, token: Address, to: Address, amount: i128) {
                    token.transfer(&env.current_contract_address(), &to, &amount);
                }
            }
        "#;

        let findings = ReserveWithdrawalAuthRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].message.contains("admin authorization"));
        assert!(findings[0].message.contains("nonce"));
    }

    #[test]
    fn accepts_admin_auth_with_nonce_guard() {
        let source = r#"
            use soroban_sdk::{Address, Env, Symbol};

            impl TreasuryContract {
                pub fn withdraw_reserve(
                    env: Env,
                    admin: Address,
                    token: Address,
                    to: Address,
                    amount: i128,
                    nonce: u64,
                ) {
                    admin.require_auth_for_args((Symbol::short("reserve"), nonce).into_val(&env));
                    assert!(!is_nonce_used(&env, nonce));
                    record_nonce(&env, nonce);
                    token.transfer(&env.current_contract_address(), &to, &amount);
                }
            }
        "#;

        let findings = ReserveWithdrawalAuthRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn flags_auth_only_with_no_nonce_guard() {
        let source = r#"
            use soroban_sdk::{Address, Env};

            impl TreasuryContract {
                pub fn sweep_treasury(env: Env, admin: Address, token: Address, to: Address, amount: i128) {
                    admin.require_auth();
                    token.transfer(&env.current_contract_address(), &to, &amount);
                }
            }
        "#;

        let findings = ReserveWithdrawalAuthRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("nonce/replay guard"));
    }
}
