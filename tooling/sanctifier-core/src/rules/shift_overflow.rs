use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects bit-shift operations whose shift amount can be greater than or equal
/// to the bit width of the value being shifted (issue #797).
///
/// In Rust a shift by `>= bit_width` panics in debug builds and is masked to the
/// low bits of the amount in release builds — either way the result is almost
/// never what the author intended. Soroban contracts run in release, so an
/// attacker-controlled shift amount silently produces the wrong value instead of
/// trapping, which can corrupt balances, bitsets, or packed storage keys.
///
/// The detector flags:
///   * a **constant** shift amount that is provably `>= bit_width` (definite
///     overflow — reported as an error), and
///   * a **non-constant / unbounded** shift amount, which is only *potentially*
///     `>= bit_width` (reported as a warning).
///
/// It suppresses the warning when the amount has been bounded within the same
/// function — either masked (`n & 0x3f`, `n % 64`) directly in the shift, or
/// guarded by a comparison against a constant somewhere in the function body
/// (`if n < 64 { .. }`, `assert!(n < 64)`, …). A constant amount that is in
/// range is never flagged.
pub struct ShiftOverflowRule;

impl ShiftOverflowRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShiftOverflowRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ShiftOverflowRule {
    fn name(&self) -> &str {
        "shift_overflow"
    }

    fn description(&self) -> &str {
        "Detects bit shifts whose amount may be greater than or equal to the operand's bit width"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = ShiftVisitor {
            violations: Vec::new(),
            current_fn: None,
            var_types: HashMap::new(),
            bounded: HashSet::new(),
            seen: HashSet::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct ShiftVisitor {
    violations: Vec<RuleViolation>,
    current_fn: Option<String>,
    /// Known integer bit widths for in-scope identifiers (from the signature and
    /// typed `let` bindings), used to size the shifted operand.
    var_types: HashMap<String, u16>,
    /// Identifiers proven bounded by a comparison / mask within the function.
    bounded: HashSet<String>,
    /// Deduplicates findings by `function:line` so a node is reported once.
    seen: HashSet<String>,
}

impl ShiftVisitor {
    fn enter_fn(&mut self, name: String, sig: &syn::Signature, block: &syn::Block) {
        self.current_fn = Some(name);
        self.var_types = collect_signature_widths(sig);
        self.bounded = collect_bounded_idents(block);
    }

    fn leave_fn(&mut self, prev_fn: Option<String>, prev_types: HashMap<String, u16>) {
        self.current_fn = prev_fn;
        self.var_types = prev_types;
        self.bounded.clear();
    }

    fn record(&mut self, severity: Severity, message: String, suggestion: String, line: usize) {
        let fn_name = match &self.current_fn {
            Some(f) => f.clone(),
            None => return,
        };
        let location = format!("{fn_name}:{line}");
        if self.seen.insert(location.clone()) {
            self.violations.push(
                RuleViolation::new("SANCT_SHIFT_OVERFLOW", severity, message, location)
                    .with_suggestion(suggestion),
            );
        }
    }

    fn inspect_shift(&mut self, left: &syn::Expr, right: &syn::Expr, op: &str, line: usize) {
        let width = width_of_expr(left, &self.var_types);
        let width_label = width.map_or_else(|| "N".to_string(), |w| w.to_string());

        match classify_amount(right, &self.bounded) {
            AmountKind::Bounded => {}
            AmountKind::Const(n) => {
                if let Some(w) = width {
                    if n >= u128::from(w) {
                        self.record(
                            Severity::Error,
                            format!(
                                "Shift '{op}' by constant {n} is >= the operand's bit width {w}"
                            ),
                            format!(
                                "Use a shift amount < {w}, or `.checked_{}(n)` which returns None on overflow",
                                checked_method(op)
                            ),
                            line,
                        );
                    }
                }
            }
            AmountKind::Unbounded => {
                self.record(
                    Severity::Warning,
                    format!(
                        "Shift '{op}' amount is unbounded and could be >= the operand's bit width ({width_label})"
                    ),
                    format!(
                        "Guard the amount (e.g. `if n < {width_label}`), mask it (`n & {}`), or use `.checked_{}(n)`",
                        mask_hint(width),
                        checked_method(op)
                    ),
                    line,
                );
            }
        }
    }
}

impl<'ast> Visit<'ast> for ShiftVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev_fn = self.current_fn.take();
        let prev_types = std::mem::take(&mut self.var_types);
        self.enter_fn(node.sig.ident.to_string(), &node.sig, &node.block);
        syn::visit::visit_item_fn(self, node);
        self.leave_fn(prev_fn, prev_types);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev_fn = self.current_fn.take();
        let prev_types = std::mem::take(&mut self.var_types);
        self.enter_fn(node.sig.ident.to_string(), &node.sig, &node.block);
        syn::visit::visit_impl_item_fn(self, node);
        self.leave_fn(prev_fn, prev_types);
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let syn::Pat::Type(pat_ty) = &node.pat {
            if let (Some(ident), Some(width)) = (pat_ident(&pat_ty.pat), width_of_type(&pat_ty.ty))
            {
                self.var_types.insert(ident, width);
            }
        }
        syn::visit::visit_local(self, node);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        let op = match node.op {
            syn::BinOp::Shl(_) => Some("<<"),
            syn::BinOp::Shr(_) => Some(">>"),
            syn::BinOp::ShlAssign(_) => Some("<<="),
            syn::BinOp::ShrAssign(_) => Some(">>="),
            _ => None,
        };
        if let Some(op) = op {
            let line = node.left.span().start().line;
            self.inspect_shift(&node.left, &node.right, op, line);
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

enum AmountKind {
    /// A literal integer shift amount with its value.
    Const(u128),
    /// The amount is masked / range-guarded and cannot exceed the width.
    Bounded,
    /// A variable or expression whose value is not provably in range.
    Unbounded,
}

fn classify_amount(expr: &syn::Expr, bounded: &HashSet<String>) -> AmountKind {
    match unwrap_expr(expr) {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => match lit.base10_parse::<u128>() {
            Ok(n) => AmountKind::Const(n),
            Err(_) => AmountKind::Unbounded,
        },
        // `n & mask` and `n % modulus` cap the amount at compile time.
        syn::Expr::Binary(bin) if matches!(bin.op, syn::BinOp::BitAnd(_) | syn::BinOp::Rem(_)) => {
            AmountKind::Bounded
        }
        syn::Expr::Path(path) => match path.path.get_ident() {
            Some(ident) if bounded.contains(&ident.to_string()) => AmountKind::Bounded,
            _ => AmountKind::Unbounded,
        },
        _ => AmountKind::Unbounded,
    }
}

/// Collect identifiers that are bounded within a function body — either compared
/// against something (`n < 64`, `n <= MAX`, …) or reduced with a mask / modulus.
fn collect_bounded_idents(block: &syn::Block) -> HashSet<String> {
    let mut collector = BoundCollector {
        bounded: HashSet::new(),
    };
    collector.visit_block(block);
    collector.bounded
}

struct BoundCollector {
    bounded: HashSet<String>,
}

impl BoundCollector {
    fn note_operand(&mut self, expr: &syn::Expr) {
        if let syn::Expr::Path(path) = unwrap_expr(expr) {
            if let Some(ident) = path.path.get_ident() {
                self.bounded.insert(ident.to_string());
            }
        }
    }
}

impl<'ast> Visit<'ast> for BoundCollector {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        match node.op {
            syn::BinOp::Lt(_)
            | syn::BinOp::Le(_)
            | syn::BinOp::Gt(_)
            | syn::BinOp::Ge(_)
            | syn::BinOp::BitAnd(_)
            | syn::BinOp::Rem(_) => {
                self.note_operand(&node.left);
                self.note_operand(&node.right);
            }
            _ => {}
        }
        syn::visit::visit_expr_binary(self, node);
    }
}

fn unwrap_expr(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(paren) => unwrap_expr(&paren.expr),
        syn::Expr::Group(group) => unwrap_expr(&group.expr),
        _ => expr,
    }
}

fn width_of_expr(expr: &syn::Expr, var_types: &HashMap<String, u16>) -> Option<u16> {
    match unwrap_expr(expr) {
        syn::Expr::Path(path) => path
            .path
            .get_ident()
            .and_then(|ident| var_types.get(&ident.to_string()).copied()),
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(lit),
            ..
        }) => width_of_str(lit.suffix()),
        syn::Expr::MethodCall(call) => width_of_expr(&call.receiver, var_types),
        _ => None,
    }
}

fn collect_signature_widths(sig: &syn::Signature) -> HashMap<String, u16> {
    sig.inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(pat_ty) => pat_ident(&pat_ty.pat).zip(width_of_type(&pat_ty.ty)),
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

fn pat_ident(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

fn width_of_type(ty: &syn::Type) -> Option<u16> {
    match ty {
        syn::Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .and_then(|segment| width_of_str(&segment.ident.to_string())),
        _ => None,
    }
}

fn width_of_str(name: &str) -> Option<u16> {
    match name {
        "i8" | "u8" => Some(8),
        "i16" | "u16" => Some(16),
        "i32" | "u32" => Some(32),
        "i64" | "u64" => Some(64),
        "i128" | "u128" => Some(128),
        "isize" | "usize" => Some(usize::BITS as u16),
        _ => None,
    }
}

fn checked_method(op: &str) -> &'static str {
    if op.starts_with("<<") {
        "shl"
    } else {
        "shr"
    }
}

fn mask_hint(width: Option<u16>) -> String {
    match width {
        Some(w) if w > 0 => (w - 1).to_string(),
        _ => "(N - 1)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unbounded_variable_shift() {
        let rule = ShiftOverflowRule::new();
        let source = r#"
            fn shift(value: u64, amount: u32) -> u64 {
                value << amount
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule_name, "SANCT_SHIFT_OVERFLOW");
        assert_eq!(violations[0].severity, Severity::Warning);
    }

    #[test]
    fn flags_shift_assign_with_variable() {
        let rule = ShiftOverflowRule::new();
        let source = r#"
            fn shift(mut value: u128, amount: u32) -> u128 {
                value >>= amount;
                value
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].message.contains(">>="));
    }

    #[test]
    fn flags_constant_amount_exceeding_width() {
        let rule = ShiftOverflowRule::new();
        let source = r#"
            fn shift(value: u32) -> u32 {
                value << 40
            }
        "#;
        let violations = rule.check(source);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].severity, Severity::Error);
    }

    #[test]
    fn ignores_constant_amount_in_range() {
        let rule = ShiftOverflowRule::new();
        let source = r#"
            fn shift(value: u64) -> u64 {
                value << 3
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn ignores_masked_amount() {
        let rule = ShiftOverflowRule::new();
        let source = r#"
            fn shift(value: u64, amount: u32) -> u64 {
                value << (amount & 63)
            }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn ignores_amount_guarded_by_comparison() {
        let rule = ShiftOverflowRule::new();
        let source = r#"
            fn shift(value: u64, amount: u32) -> u64 {
                if amount < 64 {
                    value << amount
                } else {
                    0
                }
            }
        "#;
        assert!(rule.check(source).is_empty());
    }
}
