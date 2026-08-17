use crate::finding_codes::SEP41_APPROVAL_NO_EXPIRATION;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File, FnArg, Pat};

/// Detects a SEP-41 `approve`-style entrypoint that never handles an
/// expiration ledger for the allowance it grants.
///
/// [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)'s
/// `approve` takes an `expiration_ledger: u32` and requires the allowance to
/// stop being spendable once the ledger passes it. A contract whose `approve`
/// has no expiration parameter at all — or takes one but never references it —
/// grants allowances that live forever (or that silently ignore the caller's
/// requested expiry), which is both a spec violation and a lingering-approval
/// risk if the owner forgets to revoke it.
pub struct Sep41ApprovalExpirationRule;

impl Sep41ApprovalExpirationRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sep41ApprovalExpirationRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for Sep41ApprovalExpirationRule {
    fn name(&self) -> &str {
        "sep41_approval_expiration"
    }

    fn description(&self) -> &str {
        "Detects an approve entrypoint with no expiration_ledger parameter, or one that is never used"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut finder = ApproveFinder {
            functions: Vec::new(),
        };
        finder.visit_file(&file);

        let mut violations = Vec::new();
        for function in finder.functions {
            let fn_name = function.sig.ident.to_string();
            if !fn_name.to_lowercase().contains("approve") {
                continue;
            }

            let line = function.sig.ident.span().start().line;
            let expiration_param = expiration_param_ident(&function.sig.inputs);

            match expiration_param {
                None => violations.push(
                    RuleViolation::new(
                        SEP41_APPROVAL_NO_EXPIRATION,
                        Severity::Error,
                        format!(
                            "{SEP41_APPROVAL_NO_EXPIRATION}: `{fn_name}` has no expiration_ledger parameter, so the allowance it grants never expires (SEP-41 violation)"
                        ),
                        format!("{fn_name}:{line}"),
                    )
                    .with_suggestion(
                        "Add an `expiration_ledger: u32` parameter and store/extend the allowance's TTL to it".to_string(),
                    ),
                ),
                Some(ident) => {
                    let mut usage = IdentUseVisitor {
                        target: &ident,
                        used: false,
                    };
                    usage.visit_block(&function.block);

                    if !usage.used {
                        violations.push(
                            RuleViolation::new(
                                SEP41_APPROVAL_NO_EXPIRATION,
                                Severity::Warning,
                                format!(
                                    "{SEP41_APPROVAL_NO_EXPIRATION}: `{fn_name}` accepts `{ident}` but never uses it, so the caller-requested expiration is silently ignored (SEP-41 violation)"
                                ),
                                format!("{fn_name}:{line}"),
                            )
                            .with_suggestion(
                                format!("Store `{ident}` alongside the allowance and extend its TTL to it, or reject the approval once the current ledger passes `{ident}`"),
                            ),
                        );
                    }
                }
            }
        }

        violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ApproveFinder<'ast> {
    functions: Vec<&'ast syn::ImplItemFn>,
}

impl<'ast> Visit<'ast> for ApproveFinder<'ast> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.functions.push(node);
        syn::visit::visit_impl_item_fn(self, node);
    }
}

/// Returns the identifier of the first parameter whose name contains "expir"
/// (matches `expiration_ledger`, `expiry`, ...).
fn expiration_param_ident(
    inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
) -> Option<String> {
    inputs.iter().find_map(|arg| match arg {
        FnArg::Typed(pat_type) => match &*pat_type.pat {
            Pat::Ident(pat_ident)
                if pat_ident.ident.to_string().to_lowercase().contains("expir") =>
            {
                Some(pat_ident.ident.to_string())
            }
            _ => None,
        },
        FnArg::Receiver(_) => None,
    })
}

struct IdentUseVisitor<'a> {
    target: &'a str,
    used: bool,
}

impl<'ast> Visit<'ast> for IdentUseVisitor<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if node.path.is_ident(self.target) {
            self.used = true;
        }
        syn::visit::visit_expr_path(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_approve_without_expiration_param() {
        let source = r#"
            impl Token {
                pub fn approve(env: Env, from: Address, spender: Address, amount: i128) {
                    from.require_auth();
                    write_allowance(&env, &from, &spender, amount);
                }
            }
        "#;

        let findings = Sep41ApprovalExpirationRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0]
            .message
            .contains("no expiration_ledger parameter"));
    }

    #[test]
    fn flags_approve_with_unused_expiration_param() {
        let source = r#"
            impl Token {
                pub fn approve(env: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
                    from.require_auth();
                    write_allowance(&env, &from, &spender, amount);
                }
            }
        "#;

        let findings = Sep41ApprovalExpirationRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("never uses it"));
    }

    #[test]
    fn accepts_approve_that_uses_expiration() {
        let source = r#"
            impl Token {
                pub fn approve(env: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
                    from.require_auth();
                    write_allowance(&env, &from, &spender, amount);
                    env.storage().temporary().extend_ttl(&key, 0, expiration_ledger);
                }
            }
        "#;

        let findings = Sep41ApprovalExpirationRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_unrelated_functions() {
        let source = r#"
            impl Token {
                pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
                    from.require_auth();
                    move_balance(&env, &from, &to, amount);
                }
            }
        "#;

        let findings = Sep41ApprovalExpirationRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
