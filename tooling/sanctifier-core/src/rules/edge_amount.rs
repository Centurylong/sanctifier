use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects transfer/mint/burn functions missing amount>0 and from!=to validation guards
pub struct EdgeAmountRule;

impl EdgeAmountRule {
    pub fn new() -> Self {
        Self
    }

    fn is_token_operation(fn_name: &str) -> Option<TokenOp> {
        let lower = fn_name.to_lowercase();
        if lower.contains("transfer") && !lower.contains("from") {
            Some(TokenOp::Transfer)
        } else if lower.contains("mint") {
            Some(TokenOp::Mint)
        } else if lower.contains("burn") {
            Some(TokenOp::Burn)
        } else {
            None
        }
    }

    fn has_amount_check(block: &syn::Block) -> bool {
        for stmt in &block.stmts {
            if Self::stmt_checks_amount(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_checks_amount(stmt: &syn::Stmt) -> bool {
        match stmt {
            syn::Stmt::Expr(expr, _) => Self::expr_checks_amount(expr),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    Self::expr_checks_amount(&init.expr)
                } else {
                    false
                }
            }
            syn::Stmt::Macro(m) => {
                // Check for assert! or require! macros with amount checks
                let tokens = m.mac.tokens.to_string();
                tokens.contains("amount") && (tokens.contains('>') || tokens.contains("!= 0"))
            }
            _ => false,
        }
    }

    fn expr_checks_amount(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::If(if_expr) => {
                // Check if condition includes amount check
                if Self::expr_mentions_amount_check(&if_expr.cond) {
                    return true;
                }
                // Recursively check branches
                Self::has_amount_check(&if_expr.then_branch)
                    || if_expr
                        .else_branch
                        .as_ref()
                        .map(|(_, e)| Self::expr_checks_amount(e))
                        .unwrap_or(false)
            }
            syn::Expr::Match(match_expr) => match_expr
                .arms
                .iter()
                .any(|arm| Self::expr_checks_amount(&arm.body)),
            syn::Expr::Block(block) => Self::has_amount_check(&block.block),
            syn::Expr::Binary(bin) => {
                // Check for comparisons with amount
                matches!(
                    bin.op,
                    syn::BinOp::Gt(_)
                        | syn::BinOp::Lt(_)
                        | syn::BinOp::Le(_)
                        | syn::BinOp::Ne(_)
                        | syn::BinOp::Ge(_)
                ) && (Self::expr_mentions_ident(&bin.left, "amount")
                    || Self::expr_mentions_ident(&bin.right, "amount"))
            }
            _ => false,
        }
    }

    fn expr_mentions_amount_check(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Binary(bin) => {
                matches!(
                    bin.op,
                    syn::BinOp::Gt(_)
                        | syn::BinOp::Lt(_)
                        | syn::BinOp::Le(_)
                        | syn::BinOp::Ne(_)
                        | syn::BinOp::Ge(_)
                ) && (Self::expr_mentions_ident(&bin.left, "amount")
                    || Self::expr_mentions_ident(&bin.right, "amount"))
            }
            _ => false,
        }
    }

    fn expr_mentions_ident(expr: &syn::Expr, name: &str) -> bool {
        match expr {
            syn::Expr::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident == name)
                .unwrap_or(false),
            _ => false,
        }
    }

    fn has_self_transfer_check(block: &syn::Block) -> bool {
        for stmt in &block.stmts {
            if Self::stmt_checks_self_transfer(stmt) {
                return true;
            }
        }
        false
    }

    fn stmt_checks_self_transfer(stmt: &syn::Stmt) -> bool {
        match stmt {
            syn::Stmt::Expr(expr, _) => Self::expr_checks_self_transfer(expr),
            syn::Stmt::Macro(m) => {
                let tokens = m.mac.tokens.to_string();
                (tokens.contains("from") && tokens.contains("to") && tokens.contains("!="))
                    || (tokens.contains("from") && tokens.contains("to") && tokens.contains("=="))
            }
            _ => false,
        }
    }

    fn expr_checks_self_transfer(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::If(if_expr) => {
                Self::cond_checks_from_to(&if_expr.cond)
                    || Self::has_self_transfer_check(&if_expr.then_branch)
                    || if_expr
                        .else_branch
                        .as_ref()
                        .map(|(_, e)| Self::expr_checks_self_transfer(e))
                        .unwrap_or(false)
            }
            syn::Expr::Binary(bin) => {
                matches!(bin.op, syn::BinOp::Ne(_) | syn::BinOp::Eq(_))
                    && ((Self::expr_mentions_ident(&bin.left, "from")
                        && Self::expr_mentions_ident(&bin.right, "to"))
                        || (Self::expr_mentions_ident(&bin.left, "to")
                            && Self::expr_mentions_ident(&bin.right, "from")))
            }
            syn::Expr::Block(block) => Self::has_self_transfer_check(&block.block),
            _ => false,
        }
    }

    fn cond_checks_from_to(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Binary(bin) => {
                matches!(bin.op, syn::BinOp::Ne(_) | syn::BinOp::Eq(_))
                    && ((Self::expr_mentions_ident(&bin.left, "from")
                        && Self::expr_mentions_ident(&bin.right, "to"))
                        || (Self::expr_mentions_ident(&bin.left, "to")
                            && Self::expr_mentions_ident(&bin.right, "from")))
            }
            _ => false,
        }
    }

    fn function_has_amount_param(sig: &syn::Signature) -> bool {
        sig.inputs.iter().any(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    return pat_ident.ident == "amount";
                }
            }
            false
        })
    }

    fn function_has_from_to_params(sig: &syn::Signature) -> bool {
        let mut has_from = false;
        let mut has_to = false;

        for arg in &sig.inputs {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    if pat_ident.ident == "from" {
                        has_from = true;
                    }
                    if pat_ident.ident == "to" {
                        has_to = true;
                    }
                }
            }
        }

        has_from && has_to
    }
}

impl Default for EdgeAmountRule {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenOp {
    Transfer,
    Mint,
    Burn,
}

impl Rule for EdgeAmountRule {
    fn name(&self) -> &str {
        "edge_amount"
    }

    fn description(&self) -> &str {
        "Detects transfer/mint/burn functions missing amount>0 or from!=to validation guards"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = EdgeAmountVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);

        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct EdgeAmountVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for EdgeAmountVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let fn_name = node.sig.ident.to_string();

        if let Some(op) = EdgeAmountRule::is_token_operation(&fn_name) {
            let has_amount_param = EdgeAmountRule::function_has_amount_param(&node.sig);
            let has_from_to = EdgeAmountRule::function_has_from_to_params(&node.sig);
            let has_amount_check = EdgeAmountRule::has_amount_check(&node.block);
            let has_self_check = EdgeAmountRule::has_self_transfer_check(&node.block);

            // Check for missing amount validation
            if has_amount_param && !has_amount_check {
                let message = match op {
                    TokenOp::Transfer => {
                        "Transfer function missing amount > 0 validation".to_string()
                    }
                    TokenOp::Mint => "Mint function missing amount > 0 validation".to_string(),
                    TokenOp::Burn => "Burn function missing amount > 0 validation".to_string(),
                };

                self.violations.push(
                    RuleViolation::new(
                        "edge_amount",
                        Severity::Warning,
                        message,
                        format!("{}:{}", fn_name, node.span().start().line),
                    )
                    .with_suggestion(
                        "Add validation: if amount <= 0 { panic_with_error!(...) }".to_string(),
                    ),
                );
            }

            // Check for missing self-transfer validation (only for transfer)
            if op == TokenOp::Transfer && has_from_to && !has_self_check {
                self.violations.push(
                    RuleViolation::new(
                        "edge_amount",
                        Severity::Warning,
                        "Transfer function missing from != to validation (self-transfer check)"
                            .to_string(),
                        format!("{}:{}", fn_name, node.span().start().line),
                    )
                    .with_suggestion(
                        "Add validation: if from == to { panic_with_error!(...) }".to_string(),
                    ),
                );
            }
        }

        syn::visit::visit_impl_item_fn(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_missing_amount_check_in_transfer() {
        let rule = EdgeAmountRule::new();
        let source = r#"
            impl Token {
                pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
                    let balance = get_balance(&env, from.clone());
                    set_balance(&env, from, balance - amount);
                }
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.message.contains("amount > 0")));
    }

    #[test]
    fn test_detects_missing_self_transfer_check() {
        let rule = EdgeAmountRule::new();
        let source = r#"
            impl Token {
                pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
                    if amount <= 0 {
                        panic!("Invalid amount");
                    }
                    let balance = get_balance(&env, from.clone());
                    set_balance(&env, from, balance - amount);
                }
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert!(violations.iter().any(|v| v.message.contains("from != to")));
    }

    #[test]
    fn test_allows_proper_validation() {
        let rule = EdgeAmountRule::new();
        let source = r#"
            impl Token {
                pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
                    if amount <= 0 {
                        panic!("Invalid amount");
                    }
                    if from == to {
                        panic!("Cannot transfer to self");
                    }
                    let balance = get_balance(&env, from.clone());
                    set_balance(&env, from, balance - amount);
                }
            }
        "#;
        let violations = rule.check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_detects_missing_amount_check_in_mint() {
        let rule = EdgeAmountRule::new();
        let source = r#"
            impl Token {
                pub fn mint(env: Env, to: Address, amount: i128) {
                    let balance = get_balance(&env, to.clone());
                    set_balance(&env, to, balance + amount);
                }
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert!(violations[0].message.contains("Mint"));
    }
}
