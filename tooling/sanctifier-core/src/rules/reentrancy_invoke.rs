use crate::finding_codes::REENTRANCY_INVOKE;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, File};

pub struct ReentrancyInvokeRule;

impl ReentrancyInvokeRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReentrancyInvokeRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ReentrancyInvokeRule {
    fn name(&self) -> &str {
        "reentrancy_invoke"
    }

    fn description(&self) -> &str {
        "Detects env.invoke_contract calls before state effects (CEI pattern violation)"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ReentrancyVisitor {
            violations: Vec::new(),
            test_depth: 0,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ReentrancyVisitor {
    violations: Vec<RuleViolation>,
    test_depth: usize,
}

impl ReentrancyVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ReentrancyVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_test = has_cfg_test(&node.attrs);
        if was_test {
            self.test_depth += 1;
        }
        syn::visit::visit_item_mod(self, node);
        if was_test {
            self.test_depth -= 1;
        }
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if self.in_test_module() {
            return;
        }

        for item in &node.items {
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) {
                    continue;
                }

                let fn_name = function.sig.ident.to_string();
                let mut analyzer = FnReentrancyAnalyzer::new();
                analyzer.analyze_block(&function.block);

                if let Some(violation) = analyzer.violation(fn_name) {
                    self.violations.push(violation);
                }
            }
        }
    }
}

struct FnReentrancyAnalyzer {
    first_invoke_line: Option<usize>,
    first_effect_line: Option<usize>,
}

impl FnReentrancyAnalyzer {
    fn new() -> Self {
        Self {
            first_invoke_line: None,
            first_effect_line: None,
        }
    }

    fn is_done(&self) -> bool {
        self.first_invoke_line.is_some() && self.first_effect_line.is_some()
    }

    fn analyze_block(&mut self, block: &syn::Block) {
        for stmt in &block.stmts {
            if self.is_done() {
                break;
            }
            self.analyze_stmt(stmt);
        }
    }

    fn analyze_stmt(&mut self, stmt: &syn::Stmt) {
        if self.is_done() {
            return;
        }
        match stmt {
            syn::Stmt::Expr(expr, _) => self.analyze_expr(expr),
            syn::Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    self.analyze_expr(&init.expr);
                }
            }
            _ => {}
        }
    }

    fn analyze_expr(&mut self, expr: &syn::Expr) {
        if self.is_done() {
            return;
        }
        match expr {
            syn::Expr::MethodCall(call) => {
                let method = call.method.to_string();
                if method == "invoke_contract" && is_env_receiver(&call.receiver) {
                    if self.first_invoke_line.is_none() {
                        self.first_invoke_line = Some(call.method.span().start().line);
                    }
                    if self.is_done() {
                        return;
                    }
                }
                if matches!(method.as_str(), "set" | "update" | "remove" | "try_update")
                    && is_storage_receiver(&call.receiver)
                {
                    if self.first_effect_line.is_none() {
                        self.first_effect_line = Some(call.method.span().start().line);
                    }
                    if self.is_done() {
                        return;
                    }
                }
                self.analyze_expr(&call.receiver);
                for arg in &call.args {
                    self.analyze_expr(arg);
                }
            }
            syn::Expr::Call(call) => {
                for arg in &call.args {
                    self.analyze_expr(arg);
                }
            }
            syn::Expr::If(if_expr) => {
                self.analyze_expr(&if_expr.cond);
                self.analyze_block(&if_expr.then_branch);
                if let Some((_, else_expr)) = &if_expr.else_branch {
                    self.analyze_expr(else_expr);
                }
            }
            syn::Expr::Match(match_expr) => {
                self.analyze_expr(&match_expr.expr);
                for arm in &match_expr.arms {
                    self.analyze_expr(&arm.body);
                }
            }
            syn::Expr::Block(block) => {
                self.analyze_block(&block.block);
            }
            syn::Expr::ForLoop(loop_expr) => {
                self.analyze_expr(&loop_expr.expr);
                self.analyze_block(&loop_expr.body);
            }
            syn::Expr::While(while_expr) => {
                self.analyze_expr(&while_expr.cond);
                self.analyze_block(&while_expr.body);
            }
            syn::Expr::Loop(loop_expr) => {
                self.analyze_block(&loop_expr.body);
            }
            syn::Expr::Closure(closure) => {
                self.analyze_expr(&closure.body);
            }
            _ => {}
        }
    }

    fn violation(&self, fn_name: String) -> Option<RuleViolation> {
        match (self.first_invoke_line, self.first_effect_line) {
            (Some(invoke_line), Some(effect_line)) if invoke_line < effect_line => {
                Some(
                    RuleViolation::new(
                        REENTRANCY_INVOKE,
                        Severity::Warning,
                        format!(
                            "{}: `env.invoke_contract` is called before state effects in `{}`; this violates the Checks-Effects-Interactions pattern and may enable reentrancy",
                            REENTRANCY_INVOKE,
                            fn_name
                        ),
                        format!("{}:{}", fn_name, invoke_line),
                    )
                    .with_suggestion(
                        "Move all storage writes before the env.invoke_contract call, or add a reentrancy guard.".to_string(),
                    ),
                )
            }
            _ => None,
        }
    }
}

fn is_env_receiver(receiver: &syn::Expr) -> bool {
    let rendered = quote::quote!(#receiver).to_string();
    rendered == "env"
}

fn is_storage_receiver(receiver: &syn::Expr) -> bool {
    let rendered = quote::quote!(#receiver).to_string();
    rendered.contains("storage")
        || rendered.contains("persistent")
        || rendered.contains("temporary")
        || rendered.contains("instance")
}

fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }
        match &attr.meta {
            syn::Meta::List(list) => list
                .tokens
                .to_string()
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .any(|part| part == "test"),
            _ => false,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_invoke_before_set() {
        let src = r#"
            impl C {
                pub fn transfer(env: Env, to: Address, amount: i128) {
                    env.invoke_contract(&target, &symbol_short!("transfer"), args);
                    env.storage().persistent().set(&key, &val);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule_name, REENTRANCY_INVOKE);
    }

    #[test]
    fn no_fp_when_effects_precede_invoke() {
        let src = r#"
            impl C {
                pub fn safe_transfer(env: Env, to: Address, amount: i128) {
                    env.storage().persistent().set(&key, &val);
                    env.invoke_contract(&target, &symbol_short!("transfer"), args);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert!(v.is_empty(), "effects-before-invoke must not be flagged: {v:#?}");
    }

    #[test]
    fn no_fp_when_no_invoke() {
        let src = r#"
            impl C {
                pub fn no_op(env: Env) {
                    env.storage().persistent().set(&key, &val);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert!(v.is_empty());
    }

    #[test]
    fn no_fp_when_no_writes() {
        let src = r#"
            impl C {
                pub fn query(env: Env) -> i128 {
                    env.invoke_contract(&target, &symbol_short!("balance_of"), args)
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert!(v.is_empty(), "invoke without state writes must not be flagged: {v:#?}");
    }

    #[test]
    fn flags_invoke_before_remove() {
        let src = r#"
            impl C {
                pub fn withdraw(env: Env, who: Address) {
                    env.invoke_contract(&target, &symbol_short!("transfer"), args);
                    env.storage().persistent().remove(&who);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn flags_invoke_before_update() {
        let src = r#"
            impl C {
                pub fn update(env: Env) {
                    env.invoke_contract(&target, &symbol_short!("exec"), args);
                    env.storage().instance().update(&key, &val);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn respects_multiple_invokes_first() {
        let src = r#"
            impl C {
                pub fn multi(env: Env) {
                    env.storage().instance().set(&key, &init);
                    env.storage().persistent().set(&other, &val);
                    env.invoke_contract(&target, &symbol_short!("exec"), args);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert!(v.is_empty(), "effects before invoke must not flag: {v:#?}");
    }

    #[test]
    fn skips_test_modules() {
        let src = r#"
            #[cfg(test)]
            mod test {
                impl C {
                    pub fn test_transfer(env: Env) {
                        env.invoke_contract(&target, &symbol_short!("transfer"), args);
                        env.storage().persistent().set(&key, &val);
                    }
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert!(v.is_empty(), "test modules must be skipped: {v:#?}");
    }

    #[test]
    fn flags_invoke_before_try_update() {
        let src = r#"
            impl C {
                pub fn try_update_fn(env: Env) {
                    env.invoke_contract(&target, &symbol_short!("exec"), args);
                    env.storage().instance().try_update(&key, &val);
                }
            }
        "#;
        let v = ReentrancyInvokeRule::new().check(src);
        assert_eq!(v.len(), 1);
    }
}
