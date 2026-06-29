use serde::Serialize;
use std::collections::VecDeque;
use syn::{Expr, ItemFn, Stmt};

/// Issue found by the symbolic execution prototype.
#[derive(Debug, Serialize, Clone)]
pub struct SymbolicIssue {
    pub function_name: String,
    pub description: String,
    pub location: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathStatus {
    Active,
    Reverted,
    Returned,
}

#[derive(Debug, Clone)]
pub struct PathState {
    pub conditions: Vec<String>,
    pub status: PathStatus,
}

/// A simple symbolic execution / path-enumeration prototype.
/// It operates on the `syn` AST, specifically evaluating `if` branches
/// and looking for `panic!` or `unwrap()` calls that represent reverts.
pub struct SymbolicAnalyzer {
    pub issues: Vec<SymbolicIssue>,
}

impl SymbolicAnalyzer {
    pub fn new() -> Self {
        Self { issues: Vec::new() }
    }

    pub fn analyze_function(&mut self, func: &ItemFn) {
        let fn_name = func.sig.ident.to_string();

        let initial_state = PathState {
            conditions: Vec::new(),
            status: PathStatus::Active,
        };

        let mut paths = vec![initial_state];

        for stmt in &func.block.stmts {
            paths = self.step_stmts(&paths, stmt);
        }

        let mut has_success = false;
        let mut always_reverts = true;

        for path in &paths {
            if path.status != PathStatus::Reverted {
                always_reverts = false;
            }
            if path.status == PathStatus::Active || path.status == PathStatus::Returned {
                has_success = true;
            }
        }

        // Flag entrypoint if it always reverts.
        if always_reverts && !paths.is_empty() {
            self.issues.push(SymbolicIssue {
                function_name: fn_name.clone(),
                description: "Function always reverts on all execution paths (Always-Revert)."
                    .to_string(),
                location: format!("fn {}", fn_name),
            });
        }
    }

    fn step_stmts(&self, current_paths: &[PathState], stmt: &Stmt) -> Vec<PathState> {
        let mut next_paths = Vec::new();
        for path in current_paths {
            if path.status != PathStatus::Active {
                next_paths.push(path.clone());
                continue;
            }

            match stmt {
                Stmt::Expr(expr, _) => {
                    next_paths.extend(self.step_expr(path, expr));
                }
                Stmt::Macro(syn::StmtMacro { mac, .. }) => {
                    let mac_path = quote::quote!(#mac.path).to_string();
                    if mac_path == "panic" || mac_path == "assert" {
                        let mut revert_path = path.clone();
                        revert_path.status = PathStatus::Reverted;
                        next_paths.push(revert_path);
                    } else {
                        next_paths.push(path.clone());
                    }
                }
                Stmt::Local(local) => {
                    if let Some(init) = &local.init {
                        next_paths.extend(self.step_expr(path, &init.expr));
                    } else {
                        next_paths.push(path.clone());
                    }
                }
                Stmt::Item(_) => {
                    next_paths.push(path.clone());
                }
                _ => {
                    next_paths.push(path.clone());
                }
            }
        }
        next_paths
    }

    fn step_expr(&self, path: &PathState, expr: &Expr) -> Vec<PathState> {
        let mut next_paths = Vec::new();
        match expr {
            Expr::If(expr_if) => {
                let cond_str = quote::quote!(#expr_if.cond).to_string();

                // True branch
                let mut true_path = path.clone();
                true_path.conditions.push(cond_str.clone());

                // Evaluate statements in true branch
                let mut true_paths = vec![true_path];
                for stmt in &expr_if.then_branch.stmts {
                    true_paths = self.step_stmts(&true_paths, stmt);
                }
                next_paths.extend(true_paths);

                // False branch
                let mut false_path = path.clone();
                false_path.conditions.push(format!("!({})", cond_str));

                if let Some((_, else_expr)) = &expr_if.else_branch {
                    let false_paths = self.step_expr(&false_path, else_expr);
                    next_paths.extend(false_paths);
                } else {
                    next_paths.push(false_path);
                }
            }
            Expr::Block(expr_block) => {
                let mut block_paths = vec![path.clone()];
                for stmt in &expr_block.block.stmts {
                    block_paths = self.step_stmts(&block_paths, stmt);
                }
                next_paths.extend(block_paths);
            }
            Expr::Macro(expr_macro) => {
                let mac_path = quote::quote!(#expr_macro.mac.path).to_string();
                if mac_path == "panic" || mac_path == "assert" {
                    let mut revert_path = path.clone();
                    revert_path.status = PathStatus::Reverted;
                    next_paths.push(revert_path);
                } else {
                    next_paths.push(path.clone());
                }
            }
            Expr::MethodCall(expr_method) => {
                let method = expr_method.method.to_string();
                if method == "unwrap" || method == "expect" {
                    let mut revert_path = path.clone();
                    revert_path.status = PathStatus::Reverted;
                    next_paths.push(revert_path);
                } else {
                    // Check receiver recursively
                    next_paths.extend(self.step_expr(path, &expr_method.receiver));
                }
            }
            Expr::Return(_) => {
                let mut ret_path = path.clone();
                ret_path.status = PathStatus::Returned;
                next_paths.push(ret_path);
            }
            Expr::Call(expr_call) => {
                // simple fallthrough for generic function calls
                next_paths.push(path.clone());
            }
            _ => {
                // Default: just pass the state through
                next_paths.push(path.clone());
            }
        }
        next_paths
    }
}
