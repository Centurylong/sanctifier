use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

/// Detects hardcoded addresses, secrets, or admin keys in authentication contexts
pub struct HardcodedAddrRule;

impl HardcodedAddrRule {
    pub fn new() -> Self {
        Self
    }

    /// Check if a string literal looks like a Stellar/Soroban address or secret
    fn is_address_like(s: &str) -> bool {
        // Stellar addresses start with G (public) or S (secret)
        // and are 56 characters long (base32 encoded)
        if s.len() == 56 && (s.starts_with('G') || s.starts_with('S')) {
            // Check if it's all uppercase alphanumeric (base32)
            return s
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
        }

        // Also check for hex-encoded addresses (64 hex chars)
        if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }

        // Check for common secret patterns
        if (s.contains("secret")
            || s.contains("SECRET")
            || s.contains("admin")
            || s.contains("ADMIN"))
            && s.len() > 20
        {
            return true;
        }

        false
    }

    /// Check if the literal is in an auth-related context
    fn is_auth_context(parent_fn: &Option<String>) -> bool {
        if let Some(fn_name) = parent_fn {
            let lower = fn_name.to_lowercase();
            return lower.contains("auth")
                || lower.contains("admin")
                || lower.contains("initialize")
                || lower.contains("init")
                || lower.contains("owner")
                || lower.contains("verify");
        }
        false
    }
}

impl Default for HardcodedAddrRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for HardcodedAddrRule {
    fn name(&self) -> &str {
        "hardcoded_addr"
    }

    fn description(&self) -> &str {
        "Detects hardcoded admin addresses or secret literals in authentication contexts"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let mut visitor = HardcodedAddrVisitor {
            issues: Vec::new(),
            current_fn: None,
        };
        visitor.visit_file(&file);

        visitor.issues
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct HardcodedAddrVisitor {
    issues: Vec<RuleViolation>,
    current_fn: Option<String>,
}

impl<'ast> Visit<'ast> for HardcodedAddrVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        let prev = self.current_fn.take();
        self.current_fn = Some(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.current_fn = prev;
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let prev = self.current_fn.take();
        self.current_fn = Some(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.current_fn = prev;
    }

    fn visit_expr_lit(&mut self, node: &'ast syn::ExprLit) {
        match &node.lit {
            syn::Lit::Str(lit_str) => {
                let value = lit_str.value();
                if HardcodedAddrRule::is_address_like(&value) {
                    let fn_name = self.current_fn.as_deref().unwrap_or("unknown");
                    let is_auth = HardcodedAddrRule::is_auth_context(&self.current_fn);
                    let severity = if is_auth {
                        Severity::Error
                    } else {
                        Severity::Warning
                    };

                    let message = if value.starts_with('S') {
                        "Hardcoded secret key detected - never hardcode secrets in source code"
                            .to_string()
                    } else if value.starts_with('G') {
                        "Hardcoded Stellar address detected in authentication context".to_string()
                    } else if value.contains("secret") || value.contains("SECRET") {
                        "Potential hardcoded secret detected".to_string()
                    } else {
                        "Hardcoded address-like literal detected".to_string()
                    };

                    self.issues.push(
                        RuleViolation::new(
                            "hardcoded_addr",
                            severity,
                            message,
                            format!("{}:{}", fn_name, node.span().start().line),
                        )
                        .with_suggestion(
                            "Store sensitive values in contract storage or pass as parameters"
                                .to_string(),
                        ),
                    );
                }
            }
            syn::Lit::ByteStr(lit_bytes) => {
                // Check for hardcoded byte arrays that could be keys/addresses
                let bytes = lit_bytes.value();
                // Stellar addresses are 32 bytes, Ed25519 keys are 32 bytes
                if bytes.len() == 32 || bytes.len() == 56 || bytes.len() == 64 {
                    let fn_name = self.current_fn.as_deref().unwrap_or("unknown");
                    let is_auth = HardcodedAddrRule::is_auth_context(&self.current_fn);

                    if is_auth {
                        self.issues.push(
                            RuleViolation::new(
                                "hardcoded_addr",
                                Severity::Warning,
                                "Hardcoded byte array in authentication context - potential secret key".to_string(),
                                format!("{}:{}", fn_name, node.span().start().line),
                            )
                            .with_suggestion(
                                "Store keys in contract storage or derive from parameters".to_string()
                            ),
                        );
                    }
                }
            }
            _ => {}
        }
        syn::visit::visit_expr_lit(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_stellar_public_address() {
        let rule = HardcodedAddrRule::new();
        let source = r#"
            fn initialize() {
                let admin = "GDJKFGJFKJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJG";
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert!(violations[0].message.contains("address"));
    }

    #[test]
    fn test_detects_secret_key() {
        let rule = HardcodedAddrRule::new();
        let source = r#"
            fn verify_auth() {
                let secret = "SDJKFGJFKJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJG";
            }
        "#;
        let violations = rule.check(source);
        assert!(!violations.is_empty());
        assert_eq!(violations[0].severity, Severity::Error);
    }

    #[test]
    fn test_ignores_normal_strings() {
        let rule = HardcodedAddrRule::new();
        let source = r#"
            fn get_name() {
                let name = "MyToken";
            }
        "#;
        let violations = rule.check(source);
        assert!(violations.is_empty());
    }
}
