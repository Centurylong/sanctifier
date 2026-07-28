use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects fee/interest calculations that use integer division without a minimum-fee guard,
/// allowing attackers to split transactions into micro-amounts so `amount * rate / DENOM = 0`.
pub struct FeeRoundingRule;

impl FeeRoundingRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FeeRoundingRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for FeeRoundingRule {
    fn name(&self) -> &str {
        "fee_rounding"
    }

    fn description(&self) -> &str {
        "Detects fee/interest calculations using integer division that can round to zero, enabling fee evasion via micro-transactions"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = FeeRoundingVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FeeRoundingVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FeeRoundingVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let fn_name = node.sig.ident.to_string();
        self.check_fn_block(&fn_name, &node.block);
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let fn_name = node.sig.ident.to_string();
        self.check_fn_block(&fn_name, &node.block);
        syn::visit::visit_item_fn(self, node);
    }
}

impl FeeRoundingVisitor {
    fn check_fn_block(&mut self, fn_name: &str, block: &syn::Block) {
        for stmt in &block.stmts {
            let syn::Stmt::Local(local) = stmt else {
                continue;
            };

            let var_name = match &local.pat {
                syn::Pat::Ident(p) => p.ident.to_string(),
                _ => continue,
            };

            if !is_fee_related(&var_name) {
                continue;
            }

            let Some(init) = &local.init else {
                continue;
            };

            if !is_mul_div_pattern(&init.expr) {
                continue;
            }

            if has_min_fee_guard(block, &var_name) {
                continue;
            }

            let line = local.pat.span().start().line;
            self.violations.push(
                RuleViolation::new(
                    "fee_rounding",
                    Severity::Warning,
                    format!(
                        "Fee calculation `{}` uses integer division that rounds to zero for small \
                         amounts — attackers can split transactions to evade fees",
                        var_name
                    ),
                    format!("{}:{}", fn_name, line),
                )
                .with_suggestion(
                    "Add a minimum-fee guard: `if fee == 0 && amount > 0 { fee = 1; }` \
                     or use `.max(1)` on the computed value"
                        .to_string(),
                ),
            );
        }
    }
}

/// Returns true if the variable name suggests a fee or rate calculation.
fn is_fee_related(name: &str) -> bool {
    let lower = name.to_lowercase();
    [
        "fee",
        "interest",
        "rate",
        "charge",
        "bps",
        "tax",
        "commission",
        "royalty",
        "duty",
        "premium",
    ]
    .iter()
    .any(|k| lower.contains(k))
}

/// Returns true if `expr` has the shape `A * B / LARGE_INT` (integer division of a product).
fn is_mul_div_pattern(expr: &syn::Expr) -> bool {
    if let syn::Expr::Binary(b) = expr {
        if matches!(b.op, syn::BinOp::Div(_)) && is_large_int_literal(&b.right) {
            return contains_mul(&b.left);
        }
    }
    false
}

/// Returns true if the expression is an integer literal >= 100 (a plausible fee denominator).
fn is_large_int_literal(expr: &syn::Expr) -> bool {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(n),
        ..
    }) = expr
    {
        n.base10_parse::<u64>().unwrap_or(0) >= 100
    } else {
        false
    }
}

/// Returns true if `expr` contains a `*` operator anywhere (direct or nested).
fn contains_mul(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Binary(b) => {
            matches!(b.op, syn::BinOp::Mul(_)) || contains_mul(&b.left) || contains_mul(&b.right)
        }
        syn::Expr::Paren(p) => contains_mul(&p.expr),
        _ => false,
    }
}

/// Returns true if the block contains a minimum-fee guard for `var_name`, such as:
/// - `if var_name == 0 { ... }`
/// - `let _ = var_name.max(1)` / `var_name = var_name.max(1)`
fn has_min_fee_guard(block: &syn::Block, var_name: &str) -> bool {
    for stmt in &block.stmts {
        match stmt {
            syn::Stmt::Expr(expr, _) => {
                if expr_is_guard(expr, var_name) {
                    return true;
                }
            }
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    if expr_is_max_call(&init.expr, var_name) {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn expr_is_guard(expr: &syn::Expr, var_name: &str) -> bool {
    match expr {
        syn::Expr::If(if_expr) => {
            if cond_checks_var_zero(&if_expr.cond, var_name) {
                return true;
            }
            // Recurse into branches.
            for stmt in &if_expr.then_branch.stmts {
                if let syn::Stmt::Expr(e, _) = stmt {
                    if expr_is_guard(e, var_name) {
                        return true;
                    }
                }
            }
            false
        }
        syn::Expr::Assign(a) => expr_is_max_call(&a.right, var_name),
        syn::Expr::Block(b) => {
            for stmt in &b.block.stmts {
                if let syn::Stmt::Expr(e, _) = stmt {
                    if expr_is_guard(e, var_name) {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Returns true if the condition checks `var_name == 0` (possibly within an `&&` chain).
fn cond_checks_var_zero(expr: &syn::Expr, var_name: &str) -> bool {
    if let syn::Expr::Binary(b) = expr {
        match &b.op {
            syn::BinOp::Eq(_) => {
                return (expr_is_ident(&b.left, var_name) && is_zero_literal(&b.right))
                    || (expr_is_ident(&b.right, var_name) && is_zero_literal(&b.left));
            }
            syn::BinOp::And(_) => {
                return cond_checks_var_zero(&b.left, var_name)
                    || cond_checks_var_zero(&b.right, var_name);
            }
            _ => {}
        }
    }
    false
}

fn expr_is_ident(expr: &syn::Expr, name: &str) -> bool {
    if let syn::Expr::Path(p) = expr {
        p.path
            .segments
            .last()
            .map(|s| s.ident == name)
            .unwrap_or(false)
    } else {
        false
    }
}

fn is_zero_literal(expr: &syn::Expr) -> bool {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(n),
        ..
    }) = expr
    {
        n.base10_parse::<u64>().unwrap_or(1) == 0
    } else {
        false
    }
}

/// Returns true if `expr` is a `.max(N)` call where N >= 1.
fn expr_is_max_call(expr: &syn::Expr, _var_name: &str) -> bool {
    if let syn::Expr::MethodCall(m) = expr {
        if m.method == "max" {
            if let Some(syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(n),
                ..
            })) = m.args.first()
            {
                return n.base10_parse::<u64>().unwrap_or(0) >= 1;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flags_bare_mul_div() {
        let rule = FeeRoundingRule::new();
        let source = r#"
            impl Contract {
                pub fn charge(env: Env, amount: i128, fee_bps: i128) -> i128 {
                    let fee = amount * fee_bps / 10_000;
                    amount - fee
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("fee"));
    }

    #[test]
    fn test_no_flag_with_if_zero_guard() {
        let rule = FeeRoundingRule::new();
        let source = r#"
            impl Contract {
                pub fn charge(env: Env, amount: i128, fee_bps: i128) -> i128 {
                    let mut fee = amount * fee_bps / 10_000;
                    if fee == 0 && amount > 0 {
                        fee = 1;
                    }
                    amount - fee
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn test_no_flag_with_max_in_binding() {
        let rule = FeeRoundingRule::new();
        let source = r#"
            impl Contract {
                pub fn charge(env: Env, amount: i128, fee_bps: i128) -> i128 {
                    let fee = (amount * fee_bps / 10_000).max(1);
                    amount - fee
                }
            }
        "#;
        // `.max(1)` is the outer expression so `is_mul_div_pattern` returns false.
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn test_ignores_unrelated_variable_names() {
        let rule = FeeRoundingRule::new();
        let source = r#"
            impl Contract {
                pub fn calc(env: Env, a: i128, b: i128) -> i128 {
                    let result = a * b / 10_000;
                    result
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn test_flags_interest_calculation() {
        let rule = FeeRoundingRule::new();
        let source = r#"
            impl Contract {
                pub fn accrue(env: Env, principal: i128, rate_bps: i128) -> i128 {
                    let interest = principal * rate_bps / 10_000;
                    principal + interest
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("interest"));
    }
}
