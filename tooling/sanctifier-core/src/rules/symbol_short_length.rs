use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File, LitStr};

const FINDING_CODE: &str = "SANCT_SYMBOL_SHORT_LENGTH";
const MAX_SYMBOL_SHORT_BYTES: usize = 9;

/// Detects `symbol_short!` literals that exceed Soroban's 9-byte limit.
pub struct SymbolShortLengthRule;

impl SymbolShortLengthRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SymbolShortLengthRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for SymbolShortLengthRule {
    fn name(&self) -> &str {
        "symbol_short_length"
    }

    fn description(&self) -> &str {
        "Detects symbol_short! literals longer than 9 bytes"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return vec![],
        };

        let mut visitor = SymbolShortLengthVisitor {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
struct SymbolShortLengthVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for SymbolShortLengthVisitor {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.is_ident("symbol_short") {
            if let Ok(literal) = syn::parse2::<LitStr>(node.tokens.clone()) {
                let value = literal.value();
                let byte_len = value.len();

                if byte_len > MAX_SYMBOL_SHORT_BYTES {
                    self.violations.push(
                        RuleViolation::new(
                            FINDING_CODE,
                            Severity::Warning,
                            format!(
                                "{FINDING_CODE}: symbol_short! literal `{value}` is {byte_len} bytes, exceeding the 9-byte limit"
                            ),
                            format!("symbol_short!:{}", node.path.span().start().line),
                        )
                        .with_suggestion(
                            "Use `Symbol::new(&env, \"...\")` for symbols longer than 9 bytes"
                                .to_string(),
                        ),
                    );
                }
            }
        }

        syn::visit::visit_macro(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_symbol_short_literal_longer_than_nine_bytes() {
        let source = r#"
            use soroban_sdk::symbol_short;

            const TOO_LONG: Symbol = symbol_short!("TOO_LONG_KEY");
        "#;

        let findings = SymbolShortLengthRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, FINDING_CODE);
        assert!(findings[0].message.contains("12 bytes"));
        assert!(findings[0]
            .suggestion
            .as_ref()
            .unwrap()
            .contains("Symbol::new"));
    }

    #[test]
    fn allows_symbol_short_literal_at_limit() {
        let source = r#"
            use soroban_sdk::symbol_short;

            const OK: Symbol = symbol_short!("123456789");
        "#;

        let findings = SymbolShortLengthRule::new().check(source);

        assert!(findings.is_empty());
    }

    #[test]
    fn uses_utf8_byte_length_not_character_count() {
        let source = r#"
            use soroban_sdk::symbol_short;

            const TOO_LONG: Symbol = symbol_short!("账本账本账");
        "#;

        let findings = SymbolShortLengthRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("15 bytes"));
    }
}
