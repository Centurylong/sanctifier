use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::spanned::Spanned;
use syn::visit::Visit;

const FINDING_CODE: &str = "SANCT_TTL_EXTEND_MISCONFIG";

pub struct TtlExtendMisconfigRule;

impl TtlExtendMisconfigRule {
    pub fn new() -> Self { Self }
}

impl Default for TtlExtendMisconfigRule {
    fn default() -> Self { Self::new() }
}

impl Rule for TtlExtendMisconfigRule {
    fn name(&self) -> &str { "ttl_extend_misconfig" }

    fn description(&self) -> &str {
        "Detects storage extend_ttl calls whose constant threshold is greater than or equal to extend_to"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return vec![],
        };
        let mut visitor = TtlVisitor {
            findings: Vec::new(),
            current_fn: None,
            env_bindings: HashSet::new(),
        };
        visitor.visit_file(&file);
        visitor.findings
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

struct TtlVisitor {
    findings: Vec<RuleViolation>,
    current_fn: Option<String>,
    env_bindings: HashSet<String>,
}

impl<'ast> Visit<'ast> for TtlVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let previous_fn = self.current_fn.replace(node.sig.ident.to_string());
        let previous_envs = std::mem::replace(&mut self.env_bindings, env_bindings(&node.sig));
        syn::visit::visit_item_fn(self, node);
        self.env_bindings = previous_envs;
        self.current_fn = previous_fn;
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let previous_fn = self.current_fn.replace(node.sig.ident.to_string());
        let previous_envs = std::mem::replace(&mut self.env_bindings, env_bindings(&node.sig));
        syn::visit::visit_impl_item_fn(self, node);
        self.env_bindings = previous_envs;
        self.current_fn = previous_fn;
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "extend_ttl" {
            if let Some((kind, threshold, extend_to)) = ttl_args(node, &self.env_bindings) {
                if let (Some(threshold), Some(extend_to)) =
                    (const_u128(threshold), const_u128(extend_to))
                {
                    if threshold >= extend_to {
                        let function = self.current_fn.clone().unwrap_or_else(|| "<unknown>".into());
                        self.findings.push(
                            RuleViolation::new(
                                FINDING_CODE,
                                Severity::Warning,
                                format!(
                                    "{}.extend_ttl uses threshold {threshold} >= extend_to {extend_to}; the renewal window is misconfigured",
                                    kind
                                ),
                                format!("{}:{}", function, node.span().start().line),
                            )
                            .with_suggestion(
                                "Use constants with threshold < extend_to so renewal moves the entry to a later ledger".into(),
                            ),
                        );
                    }
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn ttl_args<'a>(
    call: &'a syn::ExprMethodCall,
    env_bindings: &HashSet<String>,
) -> Option<(&'static str, &'a syn::Expr, &'a syn::Expr)> {
    let kind = storage_kind(&call.receiver, env_bindings)?;
    let args: Vec<&syn::Expr> = call.args.iter().collect();
    match (kind, args.len()) {
        ("instance", 2) => Some((kind, args[0], args[1])),
        ("persistent" | "temporary", 3) => Some((kind, args[1], args[2])),
        _ => None,
    }
}

fn storage_kind(expr: &syn::Expr, env_bindings: &HashSet<String>) -> Option<&'static str> {
    let syn::Expr::MethodCall(call) = unwrap(expr) else { return None };
    let kind = match call.method.to_string().as_str() {
        "persistent" => "persistent",
        "instance" => "instance",
        "temporary" => "temporary",
        _ => return None,
    };
    storage_root(&call.receiver, env_bindings).then_some(kind)
}

fn storage_root(expr: &syn::Expr, env_bindings: &HashSet<String>) -> bool {
    match unwrap(expr) {
        syn::Expr::MethodCall(call) if call.method == "storage" && call.args.is_empty() => {
            is_env_binding(&call.receiver, env_bindings)
        }
        syn::Expr::MethodCall(call) => storage_root(&call.receiver, env_bindings),
        _ => false,
    }
}

fn is_env_binding(expr: &syn::Expr, env_bindings: &HashSet<String>) -> bool {
    match unwrap(expr) {
        syn::Expr::Path(path) if path.path.segments.len() == 1 => {
            env_bindings.contains(&path.path.segments[0].ident.to_string())
        }
        syn::Expr::Reference(reference) => is_env_binding(&reference.expr, env_bindings),
        _ => false,
    }
}

fn env_bindings(signature: &syn::Signature) -> HashSet<String> {
    signature
        .inputs
        .iter()
        .filter_map(|argument| {
            let syn::FnArg::Typed(argument) = argument else {
                return None;
            };
            if !type_is_env(&argument.ty) {
                return None;
            }
            let syn::Pat::Ident(binding) = argument.pat.as_ref() else {
                return None;
            };
            Some(binding.ident.to_string())
        })
        .collect()
}

fn type_is_env(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "Env"),
        syn::Type::Reference(reference) => type_is_env(&reference.elem),
        syn::Type::Paren(paren) => type_is_env(&paren.elem),
        syn::Type::Group(group) => type_is_env(&group.elem),
        _ => false,
    }
}

fn unwrap(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(p) => unwrap(&p.expr),
        syn::Expr::Group(g) => unwrap(&g.expr),
        _ => expr,
    }
}

fn unsigned_primitive_cast(value: u128, ty: &syn::Type) -> Option<u128> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some()
        || path.path.leading_colon.is_some()
        || path.path.segments.len() != 1
    {
        return None;
    }

    match path.path.segments[0].ident.to_string().as_str() {
        "u8" => Some(value & u128::from(u8::MAX)),
        "u16" => Some(value & u128::from(u16::MAX)),
        "u32" => Some(value & u128::from(u32::MAX)),
        "u64" => Some(value & u128::from(u64::MAX)),
        "u128" => Some(value),
        // usize is target-width dependent; signed, float, and user-defined casts
        // are intentionally left dynamic rather than guessed.
        _ => None,
    }
}

// A shift has the type of its LHS. Only explicit fixed-width unsigned
// literal/cast operands prove a width here; other expression types stay dynamic.
fn unsigned_shift_width(expr: &syn::Expr) -> Option<u32> {
    let name = match unwrap(expr) {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) => n.suffix(),
        syn::Expr::Cast(c) => {
            let syn::Type::Path(path) = c.ty.as_ref() else { return None };
            if path.qself.is_some()
                || path.path.leading_colon.is_some()
                || path.path.segments.len() != 1
            {
                return None;
            }
            return match path.path.segments[0].ident.to_string().as_str() {
                "u8" => Some(8), "u16" => Some(16), "u32" => Some(32),
                "u64" => Some(64), "u128" => Some(128), _ => None,
            };
        }
        _ => return None,
    };
    match name {
        "u8" => Some(8), "u16" => Some(16), "u32" => Some(32),
        "u64" => Some(64), "u128" => Some(128), _ => None,
    }
}

fn const_u128(expr: &syn::Expr) -> Option<u128> {
    match unwrap(expr) {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Int(n), .. }) => {
            n.base10_parse::<u128>().ok()
        }
        syn::Expr::Cast(c) => unsigned_primitive_cast(const_u128(&c.expr)?, &c.ty),
        syn::Expr::Binary(b) => {
            let left = const_u128(&b.left)?;
            let right = const_u128(&b.right)?;
            match &b.op {
                syn::BinOp::Add(_) => left.checked_add(right),
                syn::BinOp::Sub(_) => left.checked_sub(right),
                syn::BinOp::Mul(_) => left.checked_mul(right),
                syn::BinOp::Div(_) if right != 0 => left.checked_div(right),
                syn::BinOp::Rem(_) if right != 0 => left.checked_rem(right),
                syn::BinOp::Shl(_) | syn::BinOp::Shr(_) => {
                    let width = unsigned_shift_width(&b.left)?;
                    let shift = u32::try_from(right).ok().filter(|&n| n < width)?;
                    if matches!(&b.op, syn::BinOp::Shl(_)) {
                        let mask = u128::MAX >> (128 - width);
                        left.checked_shl(shift).map(|value| value & mask)
                    } else {
                        left.checked_shr(shift)
                    }
                }
                syn::BinOp::BitAnd(_) => Some(left & right),
                syn::BinOp::BitOr(_) => Some(left | right),
                syn::BinOp::BitXor(_) => Some(left ^ right),
                _ => None,
            }
        }
        _ => None,
    }
}


#[cfg(test)]
mod receiver_tests {
    use super::*;

    #[test]
    fn accepts_renamed_env_binding() {
        let source = r#"
            fn renew(ctx: Env) {
                ctx.storage().instance().extend_ttl(200, 100);
            }
        "#;
        assert_eq!(TtlExtendMisconfigRule::new().check(source).len(), 1);
    }

    #[test]
    fn ignores_unrelated_storage_api() {
        let source = r#"
            fn renew(cache: Cache) {
                cache.storage().persistent().extend_ttl(&KEY, 200, 100);
            }
        "#;
        assert!(TtlExtendMisconfigRule::new().check(source).is_empty());
    }

    #[test]
    fn typed_shifts_preserve_unsigned_lhs_width() {
        let valid = r#"
            fn literal_lhs(ctx: Env) {
                ctx.storage().instance().extend_ttl((128u8 << 1) as u32, 100);
            }
            fn cast_lhs(ctx: Env) {
                ctx.storage().instance().extend_ttl(((128 as u8) << 1) as u32, 100);
            }
        "#;
        let findings = TtlExtendMisconfigRule::new().check(valid);
        assert!(findings.is_empty(), "valid zero thresholds were widened: {findings:?}");

        let inverted = r#"
            fn renew(ctx: Env) {
                ctx.storage().instance().extend_ttl((1u8 << 1) as u32, 1);
            }
        "#;
        let findings = TtlExtendMisconfigRule::new().check(inverted);
        assert_eq!(findings.len(), 1, "a genuine typed-shift inversion must still be reported");
        assert!(findings[0].message.contains("threshold 2 >= extend_to 1"));

        let narrowed_right_shift = syn::parse_str::<syn::Expr>("((300u16 as u8) >> 1) as u32").unwrap();
        assert_eq!(const_u128(&narrowed_right_shift), Some(22));
        for expression in ["1u8 << 8", "1u8 >> 8", "1 << 1", "1i8 << 1", "1usize << 1"] {
            let expression = syn::parse_str::<syn::Expr>(expression).unwrap();
            assert_eq!(const_u128(&expression), None, "unproven or out-of-range shift must stay dynamic");
        }
    }
}
