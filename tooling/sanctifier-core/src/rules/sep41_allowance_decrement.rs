use crate::finding_codes::SEP41_ALLOWANCE_NOT_DECREMENTED;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects a SEP-41 `transfer_from`-style entrypoint that reads a caller's
/// allowance but never writes back a decremented value.
///
/// [SEP-41](https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0041.md)
/// requires `transfer_from` to reduce the spender's allowance by the
/// transferred amount. A contract that checks the allowance (e.g. to bound the
/// transfer) but forgets the write-back lets the same approval be spent
/// repeatedly: every `transfer_from` call succeeds again for the full
/// originally-approved amount, draining far more than the owner authorized.
pub struct Sep41AllowanceDecrementRule;

impl Sep41AllowanceDecrementRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sep41AllowanceDecrementRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Method/function name fragments that read an allowance value.
const ALLOWANCE_READ_HINTS: &[&str] = &["read_allowance", "get_allowance", "check_allowance"];

/// Method/function name fragments that persist an updated allowance value.
const ALLOWANCE_WRITE_HINTS: &[&str] = &[
    "write_allowance",
    "set_allowance",
    "spend_allowance",
    "decrease_allowance",
    "update_allowance",
];

impl Rule for Sep41AllowanceDecrementRule {
    fn name(&self) -> &str {
        "sep41_allowance_decrement"
    }

    fn description(&self) -> &str {
        "Detects a transfer_from entrypoint that reads an allowance but never decrements it"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut violations = Vec::new();
        let mut finder = TransferFromFinder {
            functions: Vec::new(),
        };
        finder.visit_file(&file);

        for function in finder.functions {
            let fn_name = function.sig.ident.to_string();
            if !fn_name.to_lowercase().contains("transfer_from")
                && !fn_name.to_lowercase().contains("transferfrom")
            {
                continue;
            }

            let mut facts = AllowanceFacts::default();
            facts.visit_block(&function.block);

            if facts.reads_allowance && !facts.writes_allowance {
                violations.push(
                    RuleViolation::new(
                        SEP41_ALLOWANCE_NOT_DECREMENTED,
                        Severity::Error,
                        format!(
                            "{SEP41_ALLOWANCE_NOT_DECREMENTED}: `{fn_name}` reads the caller's allowance but never writes back a decremented value, so the same approval can be spent repeatedly (SEP-41 violation)"
                        ),
                        format!("{fn_name}:{}", function.sig.ident.span().start().line),
                    )
                    .with_suggestion(
                        "After transferring, persist `allowance - amount` (or call the SDK's allowance-spend helper) before returning".to_string(),
                    ),
                );
            }
        }

        violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct TransferFromFinder<'ast> {
    functions: Vec<&'ast syn::ImplItemFn>,
}

impl<'ast> Visit<'ast> for TransferFromFinder<'ast> {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.functions.push(node);
        syn::visit::visit_impl_item_fn(self, node);
    }
}

#[derive(Default)]
struct AllowanceFacts {
    reads_allowance: bool,
    writes_allowance: bool,
}

impl<'ast> Visit<'ast> for AllowanceFacts {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        let lower = method.to_lowercase();

        if ALLOWANCE_WRITE_HINTS
            .iter()
            .any(|hint| lower.contains(hint))
        {
            self.writes_allowance = true;
        } else if ALLOWANCE_READ_HINTS.iter().any(|hint| lower.contains(hint))
            || (lower.contains("allowance") && (method == "get" || lower.contains("read")))
        {
            self.reads_allowance = true;
        } else if method == "set" && storage_key_mentions_allowance(&node.args) {
            self.writes_allowance = true;
        }

        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func {
            if let Some(last) = path.path.segments.last() {
                let lower = last.ident.to_string().to_lowercase();
                if ALLOWANCE_WRITE_HINTS
                    .iter()
                    .any(|hint| lower.contains(hint))
                {
                    self.writes_allowance = true;
                } else if ALLOWANCE_READ_HINTS.iter().any(|hint| lower.contains(hint)) {
                    self.reads_allowance = true;
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

/// Best-effort check for `.set(&SomeAllowanceKey, ..)` / `.set(&"Allowance", ..)`
/// storage writes whose key argument's textual form mentions "allowance".
fn storage_key_mentions_allowance(
    args: &syn::punctuated::Punctuated<syn::Expr, syn::token::Comma>,
) -> bool {
    args.iter().take(1).any(|arg| {
        quote::quote!(#arg)
            .to_string()
            .to_lowercase()
            .contains("allowance")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_transfer_from_without_decrement() {
        let source = r#"
            impl Token {
                pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
                    spender.require_auth();
                    let allowance = read_allowance(&env, &from, &spender);
                    if allowance < amount {
                        panic!("insufficient allowance");
                    }
                    move_balance(&env, &from, &to, amount);
                }
            }
        "#;

        let findings = Sep41AllowanceDecrementRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].location.contains("transfer_from"));
    }

    #[test]
    fn accepts_transfer_from_that_writes_allowance() {
        let source = r#"
            impl Token {
                pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
                    spender.require_auth();
                    let allowance = read_allowance(&env, &from, &spender);
                    if allowance < amount {
                        panic!("insufficient allowance");
                    }
                    write_allowance(&env, &from, &spender, allowance - amount);
                    move_balance(&env, &from, &to, amount);
                }
            }
        "#;

        let findings = Sep41AllowanceDecrementRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_unrelated_functions() {
        let source = r#"
            impl Token {
                pub fn balance(env: Env, id: Address) -> i128 {
                    read_balance(&env, &id)
                }
            }
        "#;

        let findings = Sep41AllowanceDecrementRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_transfer_from_with_no_allowance_read() {
        let source = r#"
            impl Token {
                pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
                    spender.require_auth();
                    move_balance(&env, &from, &to, amount);
                }
            }
        "#;

        let findings = Sep41AllowanceDecrementRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
