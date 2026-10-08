use crate::baseline::{normalize_path, ErrorReprBaselineEntry};
use crate::finding_codes::ERROR_REPR_INSTABILITY;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::Item;

/// Detects breaking changes to #[repr(u32)] #[contracterror] discriminants
/// relative to the ABI snapshot stored by `sanctifier baseline`.
pub struct ErrorReprInstabilityRule {
    path: String,
    baseline: Vec<ErrorReprBaselineEntry>,
}

#[derive(Debug)]
struct CapturedVariant {
    enum_name: String,
    variant: String,
    discriminant: u32,
    line: usize,
}

impl ErrorReprInstabilityRule {
    pub fn new() -> Self {
        Self {
            path: String::new(),
            baseline: Vec::new(),
        }
    }

    pub fn with_baseline(path: &str, baseline: &[ErrorReprBaselineEntry]) -> Self {
        let path = normalize_path(path);
        let entries = baseline
            .iter()
            .filter(|entry| normalize_path(&entry.path) == path)
            .cloned()
            .collect();
        Self {
            path,
            baseline: entries,
        }
    }

    fn has_contracterror_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| attr.path().is_ident("contracterror"))
    }

    fn has_repr_u32(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if !attr.path().is_ident("repr") {
                return false;
            }
            let mut is_u32 = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("u32") {
                    is_u32 = true;
                }
                Ok(())
            });
            is_u32
        })
    }

    fn explicit_u32(expr: &syn::Expr) -> Option<u32> {
        match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Int(value),
                ..
            }) => value.base10_parse::<u32>().ok(),
            syn::Expr::Paren(paren) => Self::explicit_u32(&paren.expr),
            syn::Expr::Group(group) => Self::explicit_u32(&group.expr),
            _ => None,
        }
    }

    fn capture_items(items: &[Item], scope: &str, captured: &mut Vec<CapturedVariant>) {
        for item in items {
            // Recurse through inline modules. External mod declarations
            // are captured separately when their source files are scanned.
            if let Item::Mod(module) = item {
                if let Some((_, children)) = &module.content {
                    let nested_scope = if scope.is_empty() {
                        module.ident.to_string()
                    } else {
                        format!("{}::{}", scope, module.ident)
                    };
                    Self::capture_items(children, &nested_scope, captured);
                }
                continue;
            }
            let Item::Enum(enum_item) = item else {
                continue;
            };
            if !Self::has_contracterror_attr(&enum_item.attrs)
                || !Self::has_repr_u32(&enum_item.attrs)
            {
                continue;
            }

            // Include module namespace so similarly named Error enums in
            // different modules cannot inherit each other's baseline codes.
            let enum_name = if scope.is_empty() {
                enum_item.ident.to_string()
            } else {
                format!("{}::{}", scope, enum_item.ident)
            };
            let mut next = Some(0u32);
            for variant in &enum_item.variants {
                let value = match &variant.discriminant {
                    Some((_, expr)) => {
                        let explicit = Self::explicit_u32(expr);
                        next = explicit.and_then(|value| value.checked_add(1));
                        explicit
                    }
                    None => {
                        let implicit = next;
                        next = implicit.and_then(|value| value.checked_add(1));
                        implicit
                    }
                };
                let Some(value) = value else {
                    continue;
                };
                captured.push(CapturedVariant {
                    enum_name: enum_name.clone(),
                    variant: variant.ident.to_string(),
                    discriminant: value,
                    line: variant.span().start().line,
                });
            }
        }
    }

    fn capture_variants(source: &str) -> Vec<CapturedVariant> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };
        let mut captured = Vec::new();
        Self::capture_items(&file.items, "", &mut captured);
        captured
    }
}

impl Default for ErrorReprInstabilityRule {
    fn default() -> Self {
        Self::new()
    }
}

/// Capture ABI-relevant discriminants from one source file for persistence.
pub fn capture_error_repr_baseline(source: &str, path: &str) -> Vec<ErrorReprBaselineEntry> {
    let path = normalize_path(path);
    ErrorReprInstabilityRule::capture_variants(source)
        .into_iter()
        .map(|variant| ErrorReprBaselineEntry {
            path: path.clone(),
            enum_name: variant.enum_name,
            variant: variant.variant,
            discriminant: variant.discriminant,
        })
        .collect()
}

impl Rule for ErrorReprInstabilityRule {
    fn name(&self) -> &str {
        "error_repr_instability"
    }

    fn description(&self) -> &str {
        "Detects #[repr(u32)] contract error discriminants that changed since the stored baseline"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        if self.baseline.is_empty() {
            return Vec::new();
        }

        let previous: std::collections::HashMap<(&str, &str), u32> = self
            .baseline
            .iter()
            .map(|entry| {
                (
                    (entry.enum_name.as_str(), entry.variant.as_str()),
                    entry.discriminant,
                )
            })
            .collect();

        Self::capture_variants(source)
            .into_iter()
            .filter_map(|current| {
                let old = previous.get(&(current.enum_name.as_str(), current.variant.as_str()))?;
                if *old == current.discriminant {
                    return None;
                }

                Some(
                    RuleViolation::new(
                        ERROR_REPR_INSTABILITY,
                        Severity::Error,
                        format!(
                            "Error variant '{}::{}' changed discriminant from {} to {}",
                            current.enum_name, current.variant, old, current.discriminant
                        ),
                        format!("{}:{}:{}", self.path, current.enum_name, current.line),
                    )
                    .with_suggestion(
                        "Preserve existing error discriminants for client compatibility; append new codes instead of reordering or renumbering existing variants"
                            .to_string(),
                    ),
                )
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_a_stored_baseline() {
        let rule = ErrorReprInstabilityRule::new();
        let source = r#"
            #[contracterror]
            #[repr(u32)]
            enum Error { A = 1, B = 2 }
        "#;
        assert!(rule.check(source).is_empty());
    }

    #[test]
    fn explicit_reorder_with_stable_numbers_is_not_a_break() {
        let old = r#"
            #[contracterror]
            #[repr(u32)]
            enum Error { A = 1, B = 2 }
        "#;
        let current = r#"
            #[contracterror]
            #[repr(u32)]
            enum Error { B = 2, A = 1 }
        "#;
        let baseline = capture_error_repr_baseline(old, "src/lib.rs");
        let rule = ErrorReprInstabilityRule::with_baseline("src/lib.rs", &baseline);
        assert!(rule.check(current).is_empty());
    }

    #[test]
    fn nested_module_error_abi_is_scoped_and_detects_drift() {
        let old = r#"
            mod ledger {
                mod errors {
                    #[contracterror]
                    #[repr(u32)]
                    pub enum Error { Missing = 1, Denied = 2 }
                }
            }
            mod token {
                #[contracterror]
                #[repr(u32)]
                pub enum Error { Missing = 10, Denied = 11 }
            }
        "#;
        let new = r#"
            mod ledger {
                mod errors {
                    #[contracterror]
                    #[repr(u32)]
                    pub enum Error { Missing = 1, Denied = 3 }
                }
            }
            mod token {
                #[contracterror]
                #[repr(u32)]
                pub enum Error { Missing = 10, Denied = 11 }
            }
        "#;
        let baseline = capture_error_repr_baseline(old, "src/lib.rs");
        assert_eq!(baseline.len(), 4);
        assert!(baseline.iter().any(|e| e.enum_name == "ledger::errors::Error"));
        assert!(baseline.iter().any(|e| e.enum_name == "token::Error"));
        let rule = ErrorReprInstabilityRule::with_baseline("src/lib.rs", &baseline);
        let findings = rule.check(new);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("ledger::errors::Error::Denied"));
    }

    #[test]
    fn non_literal_discriminant_does_not_hide_later_explicit_drift() {
        let old = r#"
            #[contracterror]
            #[repr(u32)]
            enum Error { A = 1, B = 2, C = 3, D = 4 }
        "#;
        let current = r#"
            #[contracterror]
            #[repr(u32)]
            enum Error { A = SOME_CONST, B, C = 30, D }
        "#;

        let baseline = capture_error_repr_baseline(old, "src/lib.rs");
        let rule = ErrorReprInstabilityRule::with_baseline("src/lib.rs", &baseline);
        let findings = rule.check(current);

        assert_eq!(findings.len(), 2);
        assert!(findings.iter().any(|finding| {
            finding.message.contains("Error::C")
                && finding.message.contains("from 3 to 30")
        }));
        assert!(findings.iter().any(|finding| {
            finding.message.contains("Error::D")
                && finding.message.contains("from 4 to 31")
        }));
    }
}
