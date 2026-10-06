use crate::finding_codes::SIGNED_QUANTITY;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Advises when a clearly non-negative quantity is represented by a signed
/// primitive integer without an explicit non-negative guard.
pub struct SignedNonnegativeQuantityRule;

impl SignedNonnegativeQuantityRule {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for SignedNonnegativeQuantityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for SignedNonnegativeQuantityRule {
    fn name(&self) -> &str {
        "signed_nonnegative_quantity"
    }

    fn description(&self) -> &str {
        "Detects signed integer balance/amount fields and unguarded parameters where negative values are invalid"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return vec![],
        };

        let mut visitor = QuantityVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct QuantityVisitor {
    violations: Vec<RuleViolation>,
}

impl QuantityVisitor {
    fn record(&mut self, owner: &str, name: &str, ty: &syn::Type, line: usize, guarded: bool) {
        if guarded || !is_quantity_name(name) || !is_signed_integer(ty) {
            return;
        }

        let ty_text = quote::quote!(#ty).to_string();
        self.violations.push(
            RuleViolation::new(
                SIGNED_QUANTITY,
                Severity::Info,
                format!(
                    "{SIGNED_QUANTITY}: signed integer `{name}: {ty_text}` represents a non-negative balance/amount without an explicit non-negative guard"
                ),
                format!("{owner}:{line}"),
            )
            .with_suggestion(format!(
                "Prefer an unsigned integer when `{name}` cannot be negative, or reject `{name} < 0` before using the value"
            )),
        );
    }

    fn inspect_signature(&mut self, owner: &str, sig: &syn::Signature, block: &syn::Block) {
        for input in &sig.inputs {
            let syn::FnArg::Typed(pat_ty) = input else {
                continue;
            };
            let syn::Pat::Ident(ident) = &*pat_ty.pat else {
                continue;
            };

            let name = ident.ident.to_string();
            if !is_quantity_name(&name) || !is_signed_integer(&pat_ty.ty) {
                continue;
            }

            self.record(
                owner,
                &name,
                &pat_ty.ty,
                pat_ty.span().start().line,
                has_nonnegative_guard(block, &name),
            );
        }
    }
}

impl<'ast> Visit<'ast> for QuantityVisitor {
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let owner = node.ident.to_string();
        for field in &node.fields {
            if let Some(ident) = &field.ident {
                self.record(
                    &owner,
                    &ident.to_string(),
                    &field.ty,
                    field.span().start().line,
                    false,
                );
            }
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.inspect_signature(&node.sig.ident.to_string(), &node.sig, &node.block);
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.inspect_signature(&node.sig.ident.to_string(), &node.sig, &node.block);
        syn::visit::visit_impl_item_fn(self, node);
    }
}

fn is_quantity_name(name: &str) -> bool {
    let normalized = name.trim_start_matches("r#").to_ascii_lowercase();
    let parts: Vec<&str> = normalized.split('_').filter(|part| !part.is_empty()).collect();

    if parts.iter().any(|part| {
        matches!(
            *part,
            "delta" | "diff" | "difference" | "change" | "offset" | "net"
        )
    }) {
        return false;
    }

    parts
        .iter()
        .any(|part| matches!(*part, "amount" | "amounts" | "balance" | "balances"))
}

fn is_signed_integer(ty: &syn::Type) -> bool {
    let syn::Type::Path(path) = ty else {
        return false;
    };
    let Some(segment) = path.path.segments.last() else {
        return false;
    };

    matches!(
        segment.ident.to_string().as_str(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize"
    )
}

fn has_nonnegative_guard(block: &syn::Block, target: &str) -> bool {
    for stmt in &block.stmts {
        if stmt_is_nonnegative_guard(stmt, target) {
            return true;
        }
        if stmt_mentions_target(stmt, target) {
            return false;
        }
    }
    false
}

fn stmt_is_nonnegative_guard(stmt: &syn::Stmt, target: &str) -> bool {
    match stmt {
        syn::Stmt::Macro(stmt_macro) => {
            let name = stmt_macro
                .mac
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_default();
            matches!(name.as_str(), "assert" | "require" | "ensure")
                && macro_proves_nonnegative(&stmt_macro.mac, target)
        }
        syn::Stmt::Expr(expr, _) => expr_is_nonnegative_guard(expr, target),
        _ => false,
    }
}

fn expr_is_nonnegative_guard(expr: &syn::Expr, target: &str) -> bool {
    match unwrap_parens(expr) {
        syn::Expr::Macro(expr_macro) => {
            let name = expr_macro
                .mac
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string())
                .unwrap_or_default();
            matches!(name.as_str(), "assert" | "require" | "ensure")
                && macro_proves_nonnegative(&expr_macro.mac, target)
        }
        syn::Expr::If(expr_if) => {
            condition_rejects_negative(&expr_if.cond, target)
                && block_terminates(&expr_if.then_branch)
        }
        syn::Expr::Block(expr_block) => has_nonnegative_guard(&expr_block.block, target),
        _ => false,
    }
}

fn stmt_mentions_target(stmt: &syn::Stmt, target: &str) -> bool {
    let mut visitor = TargetUseVisitor {
        target,
        found: false,
    };
    visitor.visit_stmt(stmt);
    visitor.found
}

struct TargetUseVisitor<'a> {
    target: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for TargetUseVisitor<'_> {
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if !self.found
            && node.path.segments.len() == 1
            && node.path.segments[0].ident.to_string() == self.target
        {
            self.found = true;
            return;
        }
        syn::visit::visit_expr_path(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if self.found {
            return;
        }

        let tokens = node.tokens.to_string();
        if tokens
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .any(|part| part == self.target)
        {
            self.found = true;
        }
    }

    fn visit_item_fn(&mut self, _node: &'ast syn::ItemFn) {
        // Nested functions have their own scope. Their guards cannot establish
        // a property of the enclosing function's parameter.
    }
}

fn macro_proves_nonnegative(mac: &syn::Macro, target: &str) -> bool {
    let parser =
        syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
    let Ok(args) = parser.parse2(mac.tokens.clone()) else {
        return false;
    };

    args.iter()
        .any(|expr| condition_proves_nonnegative(expr, target))
}

fn condition_proves_nonnegative(expr: &syn::Expr, target: &str) -> bool {
    let expr = unwrap_parens(expr);
    let syn::Expr::Binary(binary) = expr else {
        return false;
    };

    match (
        simple_ident(&binary.left),
        zero_literal(&binary.right),
        &binary.op,
    ) {
        (Some(name), true, syn::BinOp::Ge(_)) if name == target => return true,
        _ => {}
    }

    match (
        zero_literal(&binary.left),
        simple_ident(&binary.right),
        &binary.op,
    ) {
        (true, Some(name), syn::BinOp::Le(_)) if name == target => return true,
        _ => {}
    }

    matches!(&binary.op, syn::BinOp::And(_))
        && (condition_proves_nonnegative(&binary.left, target)
            || condition_proves_nonnegative(&binary.right, target))
}

fn condition_rejects_negative(expr: &syn::Expr, target: &str) -> bool {
    let expr = unwrap_parens(expr);
    let syn::Expr::Binary(binary) = expr else {
        return false;
    };

    match (
        simple_ident(&binary.left),
        zero_literal(&binary.right),
        &binary.op,
    ) {
        (Some(name), true, syn::BinOp::Lt(_)) if name == target => return true,
        _ => {}
    }

    matches!(
        (
            zero_literal(&binary.left),
            simple_ident(&binary.right),
            &binary.op,
        ),
        (true, Some(name), syn::BinOp::Gt(_)) if name == target
    )
}

fn unwrap_parens(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(paren) => unwrap_parens(&paren.expr),
        other => other,
    }
}

fn simple_ident(expr: &syn::Expr) -> Option<String> {
    match unwrap_parens(expr) {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => {
            Some(path.path.segments[0].ident.to_string())
        }
        _ => None,
    }
}

fn zero_literal(expr: &syn::Expr) -> bool {
    matches!(
        unwrap_parens(expr),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(value),
            ..
        }) if value.base10_parse::<i128>().ok() == Some(0)
    )
}

fn block_terminates(block: &syn::Block) -> bool {
    block.stmts.last().is_some_and(stmt_terminates)
}

fn stmt_terminates(stmt: &syn::Stmt) -> bool {
    match stmt {
        syn::Stmt::Expr(expr, _) => expr_terminates(expr),
        syn::Stmt::Macro(stmt_macro) => macro_terminates(&stmt_macro.mac),
        _ => false,
    }
}

fn expr_terminates(expr: &syn::Expr) -> bool {
    match unwrap_parens(expr) {
        syn::Expr::Return(_) => true,
        syn::Expr::Macro(expr_macro) => macro_terminates(&expr_macro.mac),
        syn::Expr::Block(expr_block) => block_terminates(&expr_block.block),
        _ => false,
    }
}

fn macro_terminates(mac: &syn::Macro) -> bool {
    mac.path
        .segments
        .last()
        .map(|segment| matches!(segment.ident.to_string().as_str(), "panic" | "panic_with_error" | "bail"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_signed_quantity_field_and_unguarded_parameter() {
        let source = r#"
            struct VaultState {
                balance: i128,
                balance_delta: i128,
            }

            fn withdraw(amount: i128, fee: i128) {
                consume(amount, fee);
            }
        "#;

        let findings = SignedNonnegativeQuantityRule::new().check(source);
        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings.iter().any(|finding| finding.message.contains("balance")));
        assert!(findings.iter().any(|finding| finding.message.contains("amount")));
    }

    #[test]
    fn accepts_explicit_nonnegative_guards() {
        let source = r#"
            fn transfer(amount: i128) {
                assert!(amount >= 0);
                consume(amount);
            }

            fn mint(balance: i64) -> Result<(), Error> {
                if balance < 0 {
                    return Err(Error::Negative);
                }
                consume(balance);
                Ok(())
            }
        "#;

        assert!(SignedNonnegativeQuantityRule::new()
            .check(source)
            .is_empty());
    }

    #[test]
    fn debug_assert_does_not_count_as_a_nonnegative_guard() {
        let source = r#"
            fn withdraw(amount: i128) {
                debug_assert!(amount >= 0);
                consume(amount);
            }
        "#;

        let findings = SignedNonnegativeQuantityRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn nested_conditional_return_does_not_count_as_a_nonnegative_guard() {
        let source = r#"
            fn withdraw(amount: i128, reject_negative: bool) {
                if amount < 0 {
                    if reject_negative {
                        return;
                    }
                }
                consume(amount);
            }
        "#;

        let findings = SignedNonnegativeQuantityRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn disjunctive_assertion_does_not_count_as_a_nonnegative_guard() {
        let source = r#"
            fn withdraw(amount: i128, allow_negative: bool) {
                assert!(amount >= 0 || allow_negative);
                consume(amount);
            }
        "#;

        let findings = SignedNonnegativeQuantityRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn guard_after_use_does_not_suppress_finding() {
        let source = r#"
            fn withdraw(amount: i128) {
                consume(amount);
                assert!(amount >= 0);
            }
        "#;

        let findings = SignedNonnegativeQuantityRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn conditional_guard_does_not_suppress_finding() {
        let source = r#"
            fn withdraw(amount: i128, validate: bool) {
                if validate {
                    assert!(amount >= 0);
                }
                consume(amount);
            }
        "#;

        let findings = SignedNonnegativeQuantityRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn ignores_unsigned_and_negative_semantic_names() {
        let source = r#"
            struct State {
                balance: u128,
                amount_delta: i128,
                net_balance: i64,
            }
        "#;

        assert!(SignedNonnegativeQuantityRule::new()
            .check(source)
            .is_empty());
    }
}
