use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::BTreeSet;
use syn::visit::Visit;

const FINDING_CODE: &str = "SANCT_UNBOUNDED_INPUT";

/// Collection-shaped parameter types whose length the caller controls.
const UNBOUNDED_TYPES: [&str; 5] = ["Bytes", "Vec", "Map", "String", "BytesN"];

/// Detects public entrypoints that accept a caller-sized `Bytes`/`Vec`/`Map`
/// argument and never check its length.
///
/// The caller decides how big the argument is. Without a cap, an oversized
/// input exhausts the resource budget, and if the value is persisted it also
/// bloats state permanently at the attacker's chosen size. Neither needs a
/// clever payload — just a big one.
///
/// This complements [`crate::rules::arg_dos`] rather than duplicating it.
/// `arg_dos` fires when a collection argument is *iterated* without a cap; this
/// one fires when the length is never validated at all, which also covers
/// `Bytes` arguments that are hashed, stored, or forwarded without ever being
/// looped over.
pub struct UnboundedInputLengthRule;

impl UnboundedInputLengthRule {
    pub fn new() -> Self {
        Self
    }

    fn check_function(
        &self,
        fn_name: &str,
        visibility: &syn::Visibility,
        sig: &syn::Signature,
        block: &syn::Block,
    ) -> Vec<RuleViolation> {
        if !matches!(visibility, syn::Visibility::Public(_)) {
            return Vec::new();
        }

        let params = unbounded_params(sig);
        if params.is_empty() {
            return Vec::new();
        }

        let mut visitor = UsageVisitor {
            params: &params,
            length_checked: BTreeSet::new(),
            used: BTreeSet::new(),
        };
        visitor.visit_block(block);

        params
            .iter()
            // A cap exists somewhere in the body, so the entrypoint is bounded.
            .filter(|(name, _)| !visitor.length_checked.contains(name))
            // An argument that is never touched cannot exhaust anything; it is
            // dead weight, not a denial-of-service vector.
            .filter(|(name, _)| visitor.used.contains(name))
            .map(|(name, ty)| {
                RuleViolation::new(
                    FINDING_CODE,
                    Severity::Warning,
                    format!(
                        "{FINDING_CODE}: `{name}: {ty}` is caller-sized and its length is never checked"
                    ),
                    format!("{fn_name}:{name}"),
                )
                .with_suggestion(format!(
                    "Cap the length before using it, e.g. `if {name}.len() > MAX_{} {{ return Err(Error::InputTooLarge); }}` \
                     with MAX_{} a named constant",
                    name.to_uppercase(),
                    name.to_uppercase()
                ))
            })
            .collect()
    }
}

impl Default for UnboundedInputLengthRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for UnboundedInputLengthRule {
    fn name(&self) -> &str {
        "unbounded_input_length"
    }

    fn description(&self) -> &str {
        "Detects public entrypoints accepting caller-sized Bytes/Vec/Map arguments with no length cap."
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(f) => (*f).clone(),
            None => return vec![],
        };

        let mut visitor = FunctionVisitor {
            rule: self,
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct FunctionVisitor<'a> {
    rule: &'a UnboundedInputLengthRule,
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for FunctionVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations.extend(self.rule.check_function(
            &node.sig.ident.to_string(),
            &node.vis,
            &node.sig,
            &node.block,
        ));
        syn::visit::visit_impl_item_fn(self, node);
    }
}

struct UsageVisitor<'a> {
    params: &'a [(String, String)],
    length_checked: BTreeSet<String>,
    used: BTreeSet<String>,
}

impl UsageVisitor<'_> {
    fn is_param(&self, name: &str) -> bool {
        self.params.iter().any(|(p, _)| p == name)
    }
}

impl<'ast> Visit<'ast> for UsageVisitor<'_> {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        // `x.len()` anywhere in the body counts as the length being consulted.
        // This is deliberately generous: the rule's job is to find entrypoints
        // where the length is never looked at, not to prove the comparison is
        // the right one. Being strict here would fire on every hand-rolled
        // bound and make the detector unusable.
        if node.method == "len" {
            if let Some(name) = base_ident(&node.receiver) {
                if self.is_param(&name) {
                    self.length_checked.insert(name);
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        let name = node.to_string();
        if self.is_param(&name) {
            self.used.insert(name);
        }
    }
}

/// Public parameters whose type is one of the caller-sized collection types,
/// paired with the type name for the message.
fn unbounded_params(sig: &syn::Signature) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for input in &sig.inputs {
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(ident) = &*pat_type.pat {
                if let Some(ty) = unbounded_type_name(&pat_type.ty) {
                    out.push((ident.ident.to_string(), ty));
                }
            }
        }
    }
    out
}

fn unbounded_type_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(path) => {
            let seg = path.path.segments.last()?;
            let name = seg.ident.to_string();
            // BytesN<N> is fixed-width by construction, so it is bounded and
            // must not be reported - the length is in the type.
            if name == "BytesN" {
                return None;
            }
            UNBOUNDED_TYPES.contains(&name.as_str()).then_some(name)
        }
        syn::Type::Reference(reference) => unbounded_type_name(&reference.elem),
        _ => None,
    }
}

fn base_ident(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => path.path.get_ident().map(|i| i.to_string()),
        syn::Expr::Reference(reference) => base_ident(&reference.expr),
        syn::Expr::MethodCall(call) if call.method == "clone" => base_ident(&call.receiver),
        _ => None,
    }
}
