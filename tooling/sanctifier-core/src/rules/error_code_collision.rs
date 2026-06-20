use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::{parse_str, File, Item};

/// Detects inconsistent or duplicate discriminants in #[contracterror] enums
pub struct ErrorCodeCollisionRule;

impl ErrorCodeCollisionRule {
    pub fn new() -> Self {
        Self
    }

    fn has_contracterror_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if let syn::Meta::Path(path) = &attr.meta {
                path.is_ident("contracterror")
            } else {
                false
            }
        })
    }

    fn extract_discriminant(variant: &syn::Variant) -> Option<i64> {
        if let Some((_, expr)) = &variant.discriminant {
            if let syn::Expr::Lit(expr_lit) = expr {
                if let syn::Lit::Int(lit_int) = &expr_lit.lit {
                    return lit_int.base10_parse::<i64>().ok();
                }
            }
            // Handle negative numbers
            if let syn::Expr::Unary(unary) = expr {
                if matches!(unary.op, syn::UnOp::Neg(_)) {
                    if let syn::Expr::Lit(expr_lit) = &*unary.expr {
                        if let syn::Lit::Int(lit_int) = &expr_lit.lit {
                            if let Ok(val) = lit_int.base10_parse::<i64>() {
                                return Some(-val);
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

impl Default for ErrorCodeCollisionRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ErrorCodeCollisionRule {
    fn name(&self) -> &str {
        "error_code_collision"
    }

    fn description(&self) -> &str {
        "Detects inconsistent or duplicate discriminants in #[contracterror] enums"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut violations = Vec::new();

        for item in &file.items {
            if let Item::Enum(enum_item) = item {
                if !Self::has_contracterror_attr(&enum_item.attrs) {
                    continue;
                }

                let enum_name = enum_item.ident.to_string();
                let mut discriminants: HashMap<i64, Vec<String>> = HashMap::new();
                let mut explicit_discriminants = HashSet::new();
                let mut implicit_discriminants = HashSet::new();
                let mut current_implicit = 0i64;

                for variant in &enum_item.variants {
                    let variant_name = variant.ident.to_string();

                    if let Some(value) = Self::extract_discriminant(variant) {
                        // Explicit discriminant
                        discriminants
                            .entry(value)
                            .or_default()
                            .push(variant_name.clone());
                        explicit_discriminants.insert(value);
                        current_implicit = value + 1;
                    } else {
                        // Implicit discriminant
                        discriminants
                            .entry(current_implicit)
                            .or_default()
                            .push(variant_name.clone());
                        implicit_discriminants.insert(current_implicit);
                        current_implicit += 1;
                    }
                }

                // Check for duplicates
                for (value, variants) in &discriminants {
                    if variants.len() > 1 {
                        violations.push(
                            RuleViolation::new(
                                self.name(),
                                Severity::Error,
                                format!(
                                    "Duplicate discriminant {} in #[contracterror] enum '{}': {}",
                                    value,
                                    enum_name,
                                    variants.join(", ")
                                ),
                                format!("{}:{}", enum_name, enum_item.span().start().line),
                            )
                            .with_suggestion(
                                "Assign explicit unique discriminants to all error variants"
                                    .to_string(),
                            ),
                        );
                    }
                }

                // Check for inconsistent style (mix of explicit and implicit)
                if !explicit_discriminants.is_empty() && !implicit_discriminants.is_empty() {
                    violations.push(
                        RuleViolation::new(
                            self.name(),
                            Severity::Warning,
                            format!(
                                "Inconsistent discriminant style in #[contracterror] enum '{}': mix of explicit and implicit values",
                                enum_name
                            ),
                            format!("{}:{}", enum_name, enum_item.span().start().line),
                        )
                        .with_suggestion(
                            "Use explicit discriminants for all variants or none for consistency".to_string()
                        ),
                    );
                }

                // Check for gaps in sequential numbering (if all explicit)
                if implicit_discriminants.is_empty() && explicit_discriminants.len() > 1 {
                    let mut values: Vec<i64> = explicit_discriminants.iter().copied().collect();
                    values.sort_unstable();

                    for i in 0..values.len() - 1 {
                        if values[i + 1] - values[i] > 1 {
                            violations.push(
                                RuleViolation::new(
                                    self.name(),
                                    Severity::Info,
                                    format!(
                                        "Non-sequential discriminants in #[contracterror] enum '{}': gap between {} and {}",
                                        enum_name, values[i], values[i + 1]
                                    ),
                                    format!("{}:{}", enum_name, enum_item.span().start().line),
                                )
                                .with_suggestion(
                                    "Consider using sequential error codes for easier maintenance".to_string()
                                ),
                            );
                            break; // Only report the first gap
                        }
                    }
                }
            }
        }

        violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_duplicate_discriminants() {
        let rule = ErrorCodeCollisionRule::new();
        let source = r#"
            #[contracterror]
            pub enum Error {
                NotFound = 1,
                Invalid = 1,
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].severity, Severity::Error);
        assert!(violations[0].message.contains("Duplicate"));
    }

    #[test]
    fn test_detects_inconsistent_style() {
        let rule = ErrorCodeCollisionRule::new();
        let source = r#"
            #[contracterror]
            pub enum Error {
                NotFound = 1,
                Invalid,
                Unauthorized = 3,
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert!(violations
            .iter()
            .any(|v| v.message.contains("Inconsistent")));
    }

    #[test]
    fn test_allows_consistent_explicit_discriminants() {
        let rule = ErrorCodeCollisionRule::new();
        let source = r#"
            #[contracterror]
            pub enum Error {
                NotFound = 1,
                Invalid = 2,
                Unauthorized = 3,
            }
        "#;
        let violations = rule.check(source);
        // Should only have info about sequential numbering, no errors
        assert!(violations.iter().all(|v| v.severity != Severity::Error));
    }

    #[test]
    fn test_allows_all_implicit_discriminants() {
        let rule = ErrorCodeCollisionRule::new();
        let source = r#"
            #[contracterror]
            pub enum Error {
                NotFound,
                Invalid,
                Unauthorized,
            }
        "#;
        let violations = rule.check(source);
        assert!(violations.is_empty());
    }

    #[test]
    fn test_ignores_non_contracterror_enums() {
        let rule = ErrorCodeCollisionRule::new();
        let source = r#"
            pub enum Status {
                Active = 1,
                Inactive = 1,
            }
        "#;
        let violations = rule.check(source);
        assert!(violations.is_empty());
    }
}
