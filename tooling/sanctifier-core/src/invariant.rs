use serde::Serialize;
use syn::{spanned::Spanned, visit::Visit, Attribute, File, ItemImpl};

/// Walk a parsed `File` and collect every `#[sanctify::invariant(...)]` found
/// on `impl` blocks.
pub fn scan_invariant_attrs(source: &str, file_label: &str) -> Vec<InvariantDecl> {
    let ast: File = match syn::parse_str(source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut visitor = InvariantVisitor {
        decls: Vec::new(),
        file_label: file_label.to_string(),
    };
    visitor.visit_file(&ast);
    visitor.decls
}

struct InvariantVisitor {
    decls: Vec<InvariantDecl>,
    file_label: String,
}

impl<'ast> Visit<'ast> for InvariantVisitor {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        for attr in &node.attrs {
            if let Some(expr_str) = extract_invariant_expr(attr) {
                let contract_name = impl_self_name(node);
                let line = node.span().start().line;
                self.decls.push(InvariantDecl {
                    contract_name,
                    expr_str,
                    location: format!("{}:{}", self.file_label, line),
                });
            }
        }
        syn::visit::visit_item_impl(self, node);
    }
}

/// Return the expression string if `attr` is `#[sanctify::invariant(...)]` or
/// `#[invariant(...)]`, otherwise `None`.
fn extract_invariant_expr(attr: &Attribute) -> Option<String> {
    let path = attr.path();
    let segments: Vec<_> = path.segments.iter().map(|s| s.ident.to_string()).collect();

    let is_invariant = match segments.as_slice() {
        [name] => name == "invariant",
        [ns, name] => ns == "sanctify" && name == "invariant",
        _ => false,
    };

    if !is_invariant {
        return None;
    }

    // attr.parse_args::<syn::Expr>() gives us the inner tokens; convert back to string.
    match attr.parse_args::<syn::Expr>() {
        Ok(expr) => Some(quote::quote!(#expr).to_string()),
        Err(_) => {
            // Fall back to raw token string if parsing as Expr fails.
            if let syn::Meta::List(ml) = &attr.meta {
                Some(ml.tokens.to_string())
            } else {
                None
            }
        }
    }
}

/// Best-effort name of the impl's self-type.
fn impl_self_name(node: &ItemImpl) -> String {
    quote::quote!(#node.self_ty)
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A `#[sanctify::invariant(EXPR)]` declaration extracted from a source file.
#[derive(Debug, Clone, Serialize)]
pub struct InvariantDecl {
    /// Name of the `impl` self-type the attribute was placed on.
    pub contract_name: String,
    /// The raw invariant expression as it appears in source.
    pub expr_str: String,
    /// Human-readable location string (`file:line`).
    pub location: String,
}

/// The outcome of attempting to verify one `InvariantDecl`.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum InvariantVerifyResult {
    /// The SMT solver proved the invariant holds for all inputs.
    Proven,
    /// The SMT solver found a counterexample (the invariant can be violated).
    Refuted { counterexample: String },
    /// The solver timed out or returned unknown.
    Unknown,
    /// The invariant expression is not in a form the SMT backend can check
    /// (e.g. it calls user functions). Dispatch to Kani instead.
    Unsupported,
}
