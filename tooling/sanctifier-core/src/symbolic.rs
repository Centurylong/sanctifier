use serde::Serialize;
use syn::{parse_str, Block, Expr, File, ImplItem, Item, Stmt};

#[derive(Debug, Serialize, Clone, PartialEq, Eq)]
pub enum IssueKind {
    AlwaysRevert,
    DeadBranch,
}

#[derive(Debug, Serialize, Clone)]
pub struct SymbolicIssue {
    pub function_name: String,
    pub issue_type: IssueKind,
    pub location: String,
    pub message: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PathOutcome {
    Success,
    Revert,
}

/// Prototype path explorer over the function CFG (bounded depth)
pub fn analyze_symbolic_paths_impl(source: &str) -> Vec<SymbolicIssue> {
    let file = match parse_str::<File>(source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    let mut issues = Vec::new();

    for item in &file.items {
        if let Item::Impl(i) = item {
            for impl_item in &i.items {
                if let ImplItem::Fn(f) = impl_item {
                    if matches!(f.vis, syn::Visibility::Public(_)) {
                        let fn_name = f.sig.ident.to_string();
                        let outcome = explore_block(&f.block);

                        // If all paths lead to a revert, flag the entrypoint
                        if outcome == PathOutcome::Revert {
                            issues.push(SymbolicIssue {
                                function_name: fn_name.clone(),
                                issue_type: IssueKind::AlwaysRevert,
                                location: fn_name.clone(),
                                message: format!(
                                    "Function '{}' always reverts on all execution paths.",
                                    fn_name
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    issues
}

fn explore_block(block: &Block) -> PathOutcome {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Expr(expr, _) => {
                let out = explore_expr(expr);
                if out == PathOutcome::Revert {
                    return PathOutcome::Revert;
                }
            }
            Stmt::Local(local) => {
                if let Some(init) = &local.init {
                    let out = explore_expr(&init.expr);
                    if out == PathOutcome::Revert {
                        return PathOutcome::Revert;
                    }
                }
            }
            Stmt::Macro(mac)
                if mac.mac.path.is_ident("panic") || mac.mac.path.is_ident("unreachable") =>
            {
                return PathOutcome::Revert;
            }
            _ => {}
        }
    }
    PathOutcome::Success
}

fn explore_expr(expr: &Expr) -> PathOutcome {
    match expr {
        Expr::Macro(m) if m.mac.path.is_ident("panic") || m.mac.path.is_ident("unreachable") => {
            return PathOutcome::Revert;
        }
        Expr::MethodCall(m) => {
            let recv_out = explore_expr(&m.receiver);
            if recv_out == PathOutcome::Revert {
                return PathOutcome::Revert;
            }
            for arg in &m.args {
                if explore_expr(arg) == PathOutcome::Revert {
                    return PathOutcome::Revert;
                }
            }
        }
        Expr::Call(c) => {
            for arg in &c.args {
                if explore_expr(arg) == PathOutcome::Revert {
                    return PathOutcome::Revert;
                }
            }
        }
        Expr::Block(b) => {
            return explore_block(&b.block);
        }
        Expr::If(i) => {
            let cond_out = explore_expr(&i.cond);
            if cond_out == PathOutcome::Revert {
                return PathOutcome::Revert;
            }

            let then_out = explore_block(&i.then_branch);

            let else_out = if let Some((_, else_expr)) = &i.else_branch {
                explore_expr(else_expr)
            } else {
                PathOutcome::Success
            };

            if then_out == PathOutcome::Revert && else_out == PathOutcome::Revert {
                return PathOutcome::Revert;
            }
        }
        Expr::Match(m) => {
            let expr_out = explore_expr(&m.expr);
            if expr_out == PathOutcome::Revert {
                return PathOutcome::Revert;
            }

            let mut all_revert = true;
            for arm in &m.arms {
                if explore_expr(&arm.body) != PathOutcome::Revert {
                    all_revert = false;
                }
            }
            if all_revert && !m.arms.is_empty() {
                return PathOutcome::Revert;
            }
        }
        _ => {}
    }
    PathOutcome::Success
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_always_revert() {
        let code = r#"
            impl MyContract {
                pub fn always_fails() {
                    if condition {
                        panic!("failed");
                    } else {
                        panic!("also failed");
                    }
                }
            }
        "#;
        let issues = analyze_symbolic_paths_impl(code);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].issue_type, IssueKind::AlwaysRevert);
    }

    #[test]
    fn test_success_path() {
        let code = r#"
            impl MyContract {
                pub fn sometimes_fails() {
                    if condition {
                        panic!("failed");
                    }
                    // success path
                }
            }
        "#;
        let issues = analyze_symbolic_paths_impl(code);
        assert_eq!(issues.len(), 0);
    }
}
