use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

const FINDING_CODE: &str = "SANCT_LEDGER_SECONDS_CONFUSION";

/// Detects likely confusion between ledger-sequence units and wall-clock
/// seconds/timestamps in public contract entrypoints.
pub struct LedgerSecondsConfusionRule;

impl LedgerSecondsConfusionRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LedgerSecondsConfusionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for LedgerSecondsConfusionRule {
    fn name(&self) -> &str {
        "ledger_seconds_confusion"
    }

    fn description(&self) -> &str {
        "Detects likely ledger-vs-seconds unit confusion in time math"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = LedgerSecondsRuleVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct LedgerSecondsRuleVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for LedgerSecondsRuleVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if matches!(node.vis, syn::Visibility::Public(_)) {
            let mut function = FunctionUnitVisitor {
                function_name: node.sig.ident.to_string(),
                seen: HashSet::new(),
                violations: Vec::new(),
            };
            function.visit_block(&node.block);
            self.violations.extend(function.violations);
        }
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if matches!(node.vis, syn::Visibility::Public(_)) {
            let mut function = FunctionUnitVisitor {
                function_name: node.sig.ident.to_string(),
                seen: HashSet::new(),
                violations: Vec::new(),
            };
            function.visit_block(&node.block);
            self.violations.extend(function.violations);
        }
        syn::visit::visit_item_fn(self, node);
    }
}

struct FunctionUnitVisitor {
    function_name: String,
    seen: HashSet<(String, usize)>,
    violations: Vec<RuleViolation>,
}

impl FunctionUnitVisitor {
    fn push_violation(&mut self, line: usize, detail: &str) {
        let key = (detail.to_string(), line);
        if !self.seen.insert(key) {
            return;
        }

        self.violations.push(
            RuleViolation::new(
                FINDING_CODE,
                Severity::Info,
                format!("{FINDING_CODE}: likely ledger-vs-seconds unit confusion in {detail}"),
                format!("{}:{line}", self.function_name),
            )
            .with_suggestion(
                "Keep ledger sequence counts and wall-clock seconds/timestamps in separate variables, and convert explicitly with a documented ledger-time assumption"
                    .to_string(),
            ),
        );
    }

    fn check_expr(&mut self, expr: &syn::Expr) {
        match expr {
            syn::Expr::Binary(binary) => {
                self.check_expr(&binary.left);
                self.check_expr(&binary.right);

                if is_time_math_op(&binary.op)
                    && expr_mentions_ledger(&binary.left) != expr_mentions_ledger(&binary.right)
                    && expr_mentions_seconds(&binary.left) != expr_mentions_seconds(&binary.right)
                    && (expr_mentions_ledger(&binary.left) || expr_mentions_ledger(&binary.right))
                    && (expr_mentions_seconds(&binary.left) || expr_mentions_seconds(&binary.right))
                {
                    self.push_violation(binary.op.span().start().line, "mixed ledger/seconds math");
                }
            }
            syn::Expr::MethodCall(method_call) => {
                let method_name = method_call.method.to_string();
                if is_ledger_ttl_method(&method_name) {
                    for arg in &method_call.args {
                        if expr_mentions_seconds(arg) {
                            self.push_violation(
                                method_call.method.span().start().line,
                                "TTL/ledger API argument",
                            );
                        }
                    }
                }

                self.check_expr(&method_call.receiver);
                for arg in &method_call.args {
                    self.check_expr(arg);
                }
            }
            syn::Expr::Call(call) => {
                let call_name = call_name(&call.func);
                if call_name
                    .as_deref()
                    .is_some_and(|name| name.contains("ledger") || name.contains("ttl"))
                {
                    for arg in &call.args {
                        if expr_mentions_seconds(arg) {
                            self.push_violation(
                                call.span().start().line,
                                "TTL/ledger helper argument",
                            );
                        }
                    }
                }

                for arg in &call.args {
                    self.check_expr(arg);
                }
            }
            syn::Expr::If(if_expr) => {
                self.check_expr(&if_expr.cond);
                self.visit_block(&if_expr.then_branch);
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.check_expr(else_expr);
                }
            }
            syn::Expr::Match(match_expr) => {
                self.check_expr(&match_expr.expr);
                for arm in &match_expr.arms {
                    self.check_expr(&arm.body);
                }
            }
            syn::Expr::Block(block) => self.visit_block(&block.block),
            syn::Expr::Paren(paren) => self.check_expr(&paren.expr),
            syn::Expr::Group(group) => self.check_expr(&group.expr),
            syn::Expr::Tuple(tuple) => {
                for elem in &tuple.elems {
                    self.check_expr(elem);
                }
            }
            syn::Expr::Array(array) => {
                for elem in &array.elems {
                    self.check_expr(elem);
                }
            }
            syn::Expr::Assign(assign) => {
                self.check_expr(&assign.left);
                self.check_expr(&assign.right);
            }
            syn::Expr::Return(return_expr) => {
                if let Some(expr) = &return_expr.expr {
                    self.check_expr(expr);
                }
            }
            _ => {}
        }
    }
}

impl<'ast> Visit<'ast> for FunctionUnitVisitor {
    fn visit_stmt(&mut self, node: &'ast syn::Stmt) {
        match node {
            syn::Stmt::Expr(expr, _) => self.check_expr(expr),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.check_expr(&init.expr);
                }
            }
            syn::Stmt::Macro(macro_stmt) => {
                let tokens = macro_stmt.mac.tokens.to_string();
                if tokens_mentions_ledger(&tokens) && tokens_mentions_seconds(&tokens) {
                    self.push_violation(macro_stmt.mac.span().start().line, "guard expression");
                }
            }
            syn::Stmt::Item(_) => {}
        }
    }
}

fn is_time_math_op(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::Add(_)
            | syn::BinOp::Sub(_)
            | syn::BinOp::Lt(_)
            | syn::BinOp::Le(_)
            | syn::BinOp::Gt(_)
            | syn::BinOp::Ge(_)
            | syn::BinOp::Eq(_)
            | syn::BinOp::Ne(_)
    )
}

fn expr_mentions_ledger(expr: &syn::Expr) -> bool {
    let tokens = quote::quote!(#expr).to_string();
    tokens_mentions_ledger(&tokens)
}

fn expr_mentions_seconds(expr: &syn::Expr) -> bool {
    let tokens = quote::quote!(#expr).to_string();
    tokens_mentions_seconds(&tokens)
}

fn tokens_mentions_ledger(tokens: &str) -> bool {
    let normalized = normalize(tokens);
    [
        "ledger",
        "ledgers",
        "sequence",
        "seqnum",
        "expirationledger",
        "expireledger",
        "liveuntilledger",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword))
}

fn tokens_mentions_seconds(tokens: &str) -> bool {
    let normalized = normalize(tokens);
    [
        "second",
        "seconds",
        "timestamp",
        "duration",
        "minutes",
        "hours",
        "days",
        "unix",
        "ttlseconds",
    ]
    .iter()
    .any(|keyword| normalized.contains(keyword))
}

fn is_ledger_ttl_method(method_name: &str) -> bool {
    let normalized = normalize(method_name);
    normalized.contains("extendttl")
        || normalized.contains("liveuntilledger")
        || normalized.contains("expirationledger")
}

fn call_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| normalize(&segment.ident.to_string())),
        _ => None,
    }
}

fn normalize(tokens: &str) -> String {
    tokens
        .chars()
        .filter(|ch| *ch != '_' && !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_mixed_ledger_and_seconds_arithmetic() {
        let source = r#"
            impl Contract {
                pub fn vesting_end(current_ledger: u32, cliff_seconds: u32) -> u32 {
                    current_ledger + cliff_seconds
                }
            }
        "#;

        let findings = LedgerSecondsConfusionRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, FINDING_CODE);
    }

    #[test]
    fn flags_seconds_passed_to_ttl_api() {
        let source = r#"
            impl Contract {
                pub fn bump(env: Env, ttl_seconds: u32) {
                    env.storage().instance().extend_ttl(100, ttl_seconds);
                }
            }
        "#;

        let findings = LedgerSecondsConfusionRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("TTL"));
    }

    #[test]
    fn allows_ledger_only_math() {
        let source = r#"
            impl Contract {
                pub fn expiry(current_ledger: u32, extra_ledgers: u32) -> u32 {
                    current_ledger + extra_ledgers
                }
            }
        "#;

        let findings = LedgerSecondsConfusionRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
