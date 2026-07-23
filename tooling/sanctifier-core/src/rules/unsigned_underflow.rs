use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects unchecked subtraction (`a - b` / `a -= b`) whose left operand is a
/// known **unsigned** integer (u8/u16/u32/u64/u128/usize).
///
/// On Soroban/WASM these operations wrap in release builds, so subtracting past
/// zero silently underflows to a huge value — a classic source of balance and
/// accounting bugs (e.g. `balance - amount` when `amount > balance`). Signed
/// subtraction and the `checked_sub`/`saturating_sub` method forms are left
/// alone; this rule is deliberately unsigned-specific to keep the signal high.
pub struct UnsignedUnderflowRule;

impl UnsignedUnderflowRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnsignedUnderflowRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UnsignedUnderflowRule {
    fn name(&self) -> &str {
        "unsigned_underflow"
    }

    fn description(&self) -> &str {
        "Detects unchecked subtraction on unsigned integers that can wrap past zero (underflow)"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };
        let mut visitor = FnVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Walks each function, collects the names of its unsigned-typed bindings, then
/// flags bare subtractions whose left operand is one of those bindings.
struct FnVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FnVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let unsigned = collect_unsigned_bindings(&node.sig, &node.block);
        self.scan_block(&node.sig.ident.to_string(), &node.block, &unsigned);
        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let unsigned = collect_unsigned_bindings(&node.sig, &node.block);
        self.scan_block(&node.sig.ident.to_string(), &node.block, &unsigned);
        syn::visit::visit_item_fn(self, node);
    }
}

impl FnVisitor {
    fn scan_block(&mut self, fn_name: &str, block: &syn::Block, unsigned: &HashSet<String>) {
        let mut sub = SubVisitor {
            fn_name: fn_name.to_string(),
            unsigned,
            seen: HashSet::new(),
            violations: Vec::new(),
        };
        sub.visit_block(block);
        self.violations.append(&mut sub.violations);
    }
}

struct SubVisitor<'a> {
    fn_name: String,
    unsigned: &'a HashSet<String>,
    seen: HashSet<usize>,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for SubVisitor<'_> {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let is_sub = matches!(node.op, syn::BinOp::Sub(_) | syn::BinOp::SubAssign(_));
        if is_sub {
            if let Some(name) = ident_of(&node.left) {
                if self.unsigned.contains(&name) {
                    self.record(&name, node.span().start().line);
                }
            }
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

impl SubVisitor<'_> {
    fn record(&mut self, var: &str, line: usize) {
        if !self.seen.insert(line) {
            return;
        }
        self.violations.push(
            RuleViolation::new(
                "unsigned_underflow",
                Severity::Warning,
                format!(
                    "Unchecked subtraction on unsigned value `{}` can underflow (wraps past zero)",
                    var
                ),
                format!("{}:{}", self.fn_name, line),
            )
            .with_suggestion(format!(
                "Use `{var}.checked_sub(rhs)` and handle `None` with a typed error, \
                 or `{var}.saturating_sub(rhs)` if clamping to zero is intended"
            )),
        );
    }
}

/// The base identifier of an lvalue-ish expression, if it is a plain path like
/// `balance` (used to match against the unsigned-binding set).
fn ident_of(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(p) => p.path.get_ident().map(|i| i.to_string()),
        syn::Expr::Paren(p) => ident_of(&p.expr),
        _ => None,
    }
}

/// Names of all unsigned-typed bindings in scope: unsigned function parameters
/// plus locals declared with an explicit unsigned type annotation.
fn collect_unsigned_bindings(sig: &syn::Signature, block: &syn::Block) -> HashSet<String> {
    let mut set = HashSet::new();

    for input in &sig.inputs {
        if let syn::FnArg::Typed(pat_ty) = input {
            if is_unsigned_type(&pat_ty.ty) {
                if let syn::Pat::Ident(p) = &*pat_ty.pat {
                    set.insert(p.ident.to_string());
                }
            }
        }
    }

    let mut locals = LocalVisitor { set: &mut set };
    locals.visit_block(block);

    set
}

struct LocalVisitor<'a> {
    set: &'a mut HashSet<String>,
}

impl<'ast> Visit<'ast> for LocalVisitor<'_> {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let syn::Pat::Type(pat_ty) = &node.pat {
            if is_unsigned_type(&pat_ty.ty) {
                if let syn::Pat::Ident(p) = &*pat_ty.pat {
                    self.set.insert(p.ident.to_string());
                }
            }
        }
        syn::visit::visit_local(self, node);
    }
}

/// Whether `ty` is a primitive unsigned integer type.
fn is_unsigned_type(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return matches!(
                seg.ident.to_string().as_str(),
                "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
            );
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unsigned_param_subtraction() {
        let rule = UnsignedUnderflowRule::new();
        let source = r#"
            impl Contract {
                pub fn withdraw(balance: u64, amount: u64) -> u64 {
                    balance - amount
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("balance"));
    }

    #[test]
    fn flags_unsigned_sub_assign() {
        let rule = UnsignedUnderflowRule::new();
        let source = r#"
            impl Contract {
                pub fn spend(mut total: u128, cost: u128) -> u128 {
                    total -= cost;
                    total
                }
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("total"));
    }

    #[test]
    fn flags_unsigned_local_binding() {
        let rule = UnsignedUnderflowRule::new();
        let source = r#"
            fn f() -> u32 {
                let count: u32 = 5;
                let step: u32 = 3;
                count - step
            }
        "#;
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
        assert!(v[0].message.contains("count"));
    }

    #[test]
    fn ignores_signed_subtraction() {
        let rule = UnsignedUnderflowRule::new();
        let source = r#"
            impl Contract {
                pub fn delta(a: i128, b: i128) -> i128 {
                    a - b
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn ignores_checked_and_saturating_sub() {
        let rule = UnsignedUnderflowRule::new();
        let source = r#"
            impl Contract {
                pub fn withdraw(balance: u64, amount: u64) -> u64 {
                    balance.saturating_sub(amount)
                }
                pub fn withdraw2(balance: u64, amount: u64) -> Option<u64> {
                    balance.checked_sub(amount)
                }
            }
        "#;
        let v = rule.check(source);
        assert!(v.is_empty());
    }

    #[test]
    fn dedups_per_line() {
        let rule = UnsignedUnderflowRule::new();
        let source = r#"
            fn f(a: u64, b: u64) -> u64 {
                a - b - b
            }
        "#;
        // Two subtractions on the same line collapse to one finding.
        let v = rule.check(source);
        assert_eq!(v.len(), 1);
    }
}
