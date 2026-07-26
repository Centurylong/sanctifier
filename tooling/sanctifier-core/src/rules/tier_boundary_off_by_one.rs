use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects `if`/`else if` tier/rank ladders that compare the same variable
/// against numeric thresholds using an inconsistent mix of strict (`<`, `>`)
/// and inclusive (`<=`, `>=`) comparisons across sibling branches.
///
/// A tier ladder is expected to use one consistent boundary convention for
/// its entire length (e.g. always `<` with ascending thresholds, so each
/// value belongs to exactly one tier). Mixing `<` and `<=` on the same
/// variable across branches is the classic off-by-one signature: a boundary
/// value either matches two branches (the earlier one wins, silently
/// misassigning it) or matches none (falling through to an unintended
/// default/last branch).
pub struct TierBoundaryOffByOneRule;

impl TierBoundaryOffByOneRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TierBoundaryOffByOneRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for TierBoundaryOffByOneRule {
    fn name(&self) -> &str {
        "tier_boundary_off_by_one"
    }

    fn description(&self) -> &str {
        "Detects if/else-if boundary ladders that mix strict (<, >) and inclusive (<=, >=) \
         comparisons against the same variable, a common source of off-by-one tier/rank \
         misassignment"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = TierBoundaryVisitor {
            violations: Vec::new(),
            current_fn: None,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Which side of the comparison the ladder variable was on, and the
/// direction/strictness of the operator once normalized to "variable first".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BoundaryOp {
    Lt,
    Le,
    Gt,
    Ge,
}

impl BoundaryOp {
    /// Same conceptual direction (upper-bound-ish `<`/`<=` or lower-bound-ish
    /// `>`/`>=`) but different strictness — the specific pattern this rule
    /// flags.
    fn same_direction_different_strictness(self, other: BoundaryOp) -> bool {
        matches!(
            (self, other),
            (BoundaryOp::Lt, BoundaryOp::Le)
                | (BoundaryOp::Le, BoundaryOp::Lt)
                | (BoundaryOp::Gt, BoundaryOp::Ge)
                | (BoundaryOp::Ge, BoundaryOp::Gt)
        )
    }

    fn as_str(self) -> &'static str {
        match self {
            BoundaryOp::Lt => "<",
            BoundaryOp::Le => "<=",
            BoundaryOp::Gt => ">",
            BoundaryOp::Ge => ">=",
        }
    }
}

struct BoundaryBranch {
    op: BoundaryOp,
    threshold: String,
    span: proc_macro2::Span,
}

struct TierBoundaryVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
}

impl TierBoundaryVisitor {
    /// Walk an `if`/`else if` chain rooted at `if_expr`, collecting every
    /// branch that compares the *same* simple variable against a numeric
    /// literal threshold. Stops collecting (but keeps recursing into bodies)
    /// once a branch's condition doesn't match that shape, or compares a
    /// different variable — a ladder is only as suspicious as the
    /// contiguous run of branches that share a subject.
    fn walk_if_chain(&mut self, root: &syn::ExprIf) {
        let mut ladder_var: Option<String> = None;
        let mut branches: Vec<BoundaryBranch> = Vec::new();
        let mut current = root;

        loop {
            // Recurse into the branch body regardless, so nested ladders are
            // still found.
            self.visit_block(&current.then_branch);

            if let Some((var, boundary)) = boundary_check(&current.cond) {
                match &ladder_var {
                    None => ladder_var = Some(var),
                    Some(existing) if *existing == var => {}
                    Some(_) => break,
                }
                branches.push(BoundaryBranch {
                    op: boundary.0,
                    threshold: boundary.1,
                    span: current.cond.span(),
                });
            } else {
                break;
            }

            match &current.else_branch {
                Some((_, else_expr)) => match else_expr.as_ref() {
                    syn::Expr::If(next_if) => current = next_if,
                    other => {
                        self.visit_expr(other);
                        break;
                    }
                },
                None => break,
            }
        }

        self.report_if_inconsistent(&ladder_var, &branches);
    }

    fn report_if_inconsistent(&mut self, ladder_var: &Option<String>, branches: &[BoundaryBranch]) {
        let Some(var) = ladder_var else { return };
        if branches.len() < 2 {
            return;
        }

        for i in 1..branches.len() {
            let prev = &branches[i - 1];
            let curr = &branches[i];
            if prev.op.same_direction_different_strictness(curr.op) {
                let fn_name = self
                    .current_fn
                    .clone()
                    .unwrap_or_else(|| "<unknown>".to_string());
                let line = curr.span.start().line;
                self.violations.push(
                    RuleViolation::new(
                        "tier_boundary_off_by_one",
                        Severity::Info,
                        format!(
                            "Boundary ladder on `{var}` mixes `{}` (`{} {} {}`) and `{}` \
                             (`{} {} {}`) across sibling branches — a likely off-by-one that \
                             either double-matches or skips the boundary value",
                            prev.op.as_str(),
                            var,
                            prev.op.as_str(),
                            prev.threshold,
                            curr.op.as_str(),
                            var,
                            curr.op.as_str(),
                            curr.threshold,
                        ),
                        format!("{fn_name}:{line}"),
                    )
                    .with_suggestion(format!(
                        "Use one consistent comparison operator (`<` or `<=`, but not both) for \
                         every branch of the `{var}` boundary ladder"
                    )),
                );
            }
        }
    }
}

impl<'ast> Visit<'ast> for TierBoundaryVisitor {
    fn visit_expr_if(&mut self, node: &'ast syn::ExprIf) {
        self.walk_if_chain(node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev_fn = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = prev_fn;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev_fn = self.current_fn.replace(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.current_fn = prev_fn;
    }
}

/// If `cond` is a simple `var OP literal` or `literal OP var` comparison
/// (optionally through a single layer of parens), returns the ladder
/// variable's name plus a normalized (variable-first) operator and the
/// threshold's literal text.
fn boundary_check(cond: &syn::Expr) -> Option<(String, (BoundaryOp, String))> {
    let cond = unwrap_paren(cond);
    let syn::Expr::Binary(bin) = cond else {
        return None;
    };

    let left_ident = simple_ident(&bin.left);
    let right_ident = simple_ident(&bin.right);
    let left_lit = numeric_literal(&bin.right);
    let right_lit = numeric_literal(&bin.left);

    if let (Some(var), Some(threshold)) = (left_ident, left_lit) {
        let op = match bin.op {
            syn::BinOp::Lt(_) => BoundaryOp::Lt,
            syn::BinOp::Le(_) => BoundaryOp::Le,
            syn::BinOp::Gt(_) => BoundaryOp::Gt,
            syn::BinOp::Ge(_) => BoundaryOp::Ge,
            _ => return None,
        };
        return Some((var, (op, threshold)));
    }

    if let (Some(var), Some(threshold)) = (right_ident, right_lit) {
        // Threshold appears on the left (`50 < score`); normalize to
        // variable-first by flipping the operator's direction.
        let op = match bin.op {
            syn::BinOp::Lt(_) => BoundaryOp::Gt,
            syn::BinOp::Le(_) => BoundaryOp::Ge,
            syn::BinOp::Gt(_) => BoundaryOp::Lt,
            syn::BinOp::Ge(_) => BoundaryOp::Le,
            _ => return None,
        };
        return Some((var, (op, threshold)));
    }

    None
}

fn unwrap_paren(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(p) => unwrap_paren(&p.expr),
        other => other,
    }
}

fn simple_ident(expr: &syn::Expr) -> Option<String> {
    match unwrap_paren(expr) {
        syn::Expr::Path(p) if p.path.segments.len() == 1 => {
            Some(p.path.segments[0].ident.to_string())
        }
        syn::Expr::Field(f) => Some(quote::quote!(#f).to_string()),
        _ => None,
    }
}

fn numeric_literal(expr: &syn::Expr) -> Option<String> {
    match unwrap_paren(expr) {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(n),
            ..
        }) => Some(n.base10_digits().to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_mixed_lt_and_le_ladder() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn tier_of(env: Env, score: u32) -> Tier {
                    if score < 50 {
                        Tier::Bronze
                    } else if score <= 80 {
                        Tier::Silver
                    } else {
                        Tier::Gold
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("score"));
        assert!(v[0].message.contains("<"));
        assert!(v[0].message.contains("<="));
    }

    #[test]
    fn flags_mixed_gt_and_ge_ladder() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn rank_of(env: Env, points: i128) -> Rank {
                    if points > 1000 {
                        Rank::Diamond
                    } else if points >= 500 {
                        Rank::Platinum
                    } else {
                        Rank::Standard
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("points"));
    }

    #[test]
    fn ignores_consistent_lt_ladder() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn tier_of(env: Env, score: u32) -> Tier {
                    if score < 50 {
                        Tier::Bronze
                    } else if score < 80 {
                        Tier::Silver
                    } else {
                        Tier::Gold
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_consistent_le_ladder() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn tier_of(env: Env, score: u32) -> Tier {
                    if score <= 50 {
                        Tier::Bronze
                    } else if score <= 80 {
                        Tier::Silver
                    } else {
                        Tier::Gold
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_single_branch_if() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn check(env: Env, score: u32) -> bool {
                    if score < 50 {
                        return false;
                    }
                    true
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_ladder_over_different_variables() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn classify(env: Env, score: u32, points: u32) -> Tier {
                    if score < 50 {
                        Tier::Bronze
                    } else if points <= 80 {
                        Tier::Silver
                    } else {
                        Tier::Gold
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn threshold_on_left_side_is_normalized() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn tier_of(env: Env, score: u32) -> Tier {
                    if 50 > score {
                        Tier::Bronze
                    } else if 80 >= score {
                        Tier::Silver
                    } else {
                        Tier::Gold
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn finds_nested_ladder_inside_branch_body() {
        let rule = TierBoundaryOffByOneRule::new();
        let source = r#"
            impl Contract {
                pub fn classify(env: Env, active: bool, score: u32) -> Tier {
                    if active {
                        if score < 50 {
                            Tier::Bronze
                        } else if score <= 80 {
                            Tier::Silver
                        } else {
                            Tier::Gold
                        }
                    } else {
                        Tier::Inactive
                    }
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }
}
