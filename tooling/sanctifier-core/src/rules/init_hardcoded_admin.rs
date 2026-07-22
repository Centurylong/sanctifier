use crate::finding_codes::INIT_HARDCODED_ADMIN;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects hardcoded admin addresses or default literal values in initialization functions.
pub struct InitHardcodedAdminRule;

impl InitHardcodedAdminRule {
    pub fn new() -> Self {
        Self
    }

    /// Check if a function name looks like an initialization function
    fn is_init_function_name(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("init") || lower.contains("initialize")
    }

    /// Check if a string literal looks like a Stellar public/secret address or hardcoded admin address
    fn is_admin_address_literal(s: &str) -> bool {
        // Stellar public (G...) or secret (S...) address (56 chars base32)
        if s.len() == 56 && (s.starts_with('G') || s.starts_with('S')) {
            if s.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            {
                return true;
            }
        }

        // Hex-encoded address (64 hex characters)
        if s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit()) {
            return true;
        }

        // Dummy/default admin address placeholders (all zero/dummy 56-char base32 address strings)
        if s == "G0000000000000000000000000000000000000000000000000000000"
            || s == "0000000000000000000000000000000000000000000000000000000000000000"
        {
            return true;
        }

        false
    }

    /// Check if function signature accepts an admin address parameter (e.g. `admin: Address` or `owner: Address`)
    fn has_admin_param(sig: &syn::Signature) -> bool {
        sig.inputs.iter().any(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                let is_admin_name = match &*pat_type.pat {
                    syn::Pat::Ident(pat_ident) => {
                        let name = pat_ident.ident.to_string().to_lowercase();
                        name.contains("admin") || name.contains("owner") || name.contains("auth")
                    }
                    _ => false,
                };
                let is_address_type = type_is_address(&pat_type.ty);
                is_admin_name || is_address_type
            } else {
                false
            }
        })
    }
}

fn type_is_address(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "Address"),
        syn::Type::Reference(reference) => type_is_address(&reference.elem),
        _ => false,
    }
}

impl Default for InitHardcodedAdminRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for InitHardcodedAdminRule {
    fn name(&self) -> &str {
        "init_hardcoded_admin"
    }

    fn description(&self) -> &str {
        "Detects hardcoded admin addresses or default literals in initialization functions"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = InitAdminVisitor {
            violations: Vec::new(),
            suppressions: suppressions(source),
            test_depth: 0,
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct InitAdminVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl InitAdminVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for InitAdminVisitor {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let was_test = has_cfg_test(&node.attrs);
        if was_test {
            self.test_depth += 1;
        }

        syn::visit::visit_item_mod(self, node);

        if was_test {
            self.test_depth -= 1;
        }
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if self.in_test_module() {
            syn::visit::visit_impl_item_fn(self, node);
            return;
        }

        let fn_name = node.sig.ident.to_string();
        if InitHardcodedAdminRule::is_init_function_name(&fn_name) {
            let has_admin_arg = InitHardcodedAdminRule::has_admin_param(&node.sig);
            let mut fn_visitor = InitFnBodyVisitor {
                fn_name,
                has_admin_arg,
                issues: Vec::new(),
            };
            fn_visitor.visit_block(&node.block);

            self.violations.extend(
                fn_visitor
                    .issues
                    .into_iter()
                    .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                    .map(InitAdminIssue::into_violation),
            );
        }

        syn::visit::visit_impl_item_fn(self, node);
    }

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if self.in_test_module() {
            syn::visit::visit_item_fn(self, node);
            return;
        }

        let fn_name = node.sig.ident.to_string();
        if InitHardcodedAdminRule::is_init_function_name(&fn_name) {
            let has_admin_arg = InitHardcodedAdminRule::has_admin_param(&node.sig);
            let mut fn_visitor = InitFnBodyVisitor {
                fn_name,
                has_admin_arg,
                issues: Vec::new(),
            };
            fn_visitor.visit_block(&node.block);

            self.violations.extend(
                fn_visitor
                    .issues
                    .into_iter()
                    .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                    .map(InitAdminIssue::into_violation),
            );
        }

        syn::visit::visit_item_fn(self, node);
    }
}

struct InitFnBodyVisitor {
    fn_name: String,
    has_admin_arg: bool,
    issues: Vec<InitAdminIssue>,
}

impl<'ast> Visit<'ast> for InitFnBodyVisitor {
    fn visit_expr_lit(&mut self, node: &'ast syn::ExprLit) {
        let line = node.span().start().line;
        match &node.lit {
            syn::Lit::Str(lit_str) => {
                let val = lit_str.value();
                if InitHardcodedAdminRule::is_admin_address_literal(&val) {
                    let is_secret = val.starts_with('S');
                    self.issues.push(InitAdminIssue {
                        fn_name: self.fn_name.clone(),
                        line,
                        is_secret,
                        detail: format!(
                            "Hardcoded address literal `{}` in initialization function",
                            val
                        ),
                    });
                }
            }
            syn::Lit::ByteStr(lit_bytes) => {
                let bytes = lit_bytes.value();
                if (bytes.len() == 32 || bytes.len() == 56 || bytes.len() == 64)
                    && !self.has_admin_arg
                {
                    self.issues.push(InitAdminIssue {
                        fn_name: self.fn_name.clone(),
                        line,
                        is_secret: false,
                        detail: "Hardcoded byte array literal in initialization function without formal admin parameter".to_string(),
                    });
                }
            }
            _ => {}
        }
        syn::visit::visit_expr_lit(self, node);
    }

    fn visit_expr_repeat(&mut self, node: &'ast syn::ExprRepeat) {
        // e.g. [0u8; 32] or [0; 32]
        if !self.has_admin_arg {
            let line = node.span().start().line;
            if let syn::Expr::Lit(lit) = &*node.len {
                if let syn::Lit::Int(int_lit) = &lit.lit {
                    if let Ok(val) = int_lit.base10_parse::<usize>() {
                        if val == 32 || val == 56 || val == 64 {
                            self.issues.push(InitAdminIssue {
                                fn_name: self.fn_name.clone(),
                                line,
                                is_secret: false,
                                detail:
                                    "Hardcoded default array pattern in initialization function"
                                        .to_string(),
                            });
                        }
                    }
                }
            }
        }
        syn::visit::visit_expr_repeat(self, node);
    }
}

struct InitAdminIssue {
    fn_name: String,
    line: usize,
    is_secret: bool,
    detail: String,
}

impl InitAdminIssue {
    fn into_violation(self) -> RuleViolation {
        let severity = if self.is_secret {
            Severity::Error
        } else {
            Severity::Warning
        };

        RuleViolation::new(
            INIT_HARDCODED_ADMIN,
            severity,
            format!(
                "{INIT_HARDCODED_ADMIN}: {} `{}`",
                self.detail, self.fn_name
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(
            "Require the admin address as a formal parameter (e.g., admin: Address) instead of hardcoding address literals or default values."
                .to_string(),
        )
    }
}

fn has_cfg_test(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("cfg") {
            return false;
        }

        match &attr.meta {
            syn::Meta::List(list) => list
                .tokens
                .to_string()
                .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                .any(|part| part == "test"),
            _ => false,
        }
    })
}

fn suppressions(source: &str) -> Vec<usize> {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.contains("sanctifier:ignore[SANCT_INIT_HARDCODED_ADMIN]")
                || line.contains("sanctifier:ignore[init_hardcoded_admin]")
            {
                Some(index + 1)
            } else {
                None
            }
        })
        .collect()
}

fn is_suppressed(suppressions: &[usize], line: usize) -> bool {
    suppressions
        .iter()
        .any(|suppressed_line| *suppressed_line == line || *suppressed_line + 1 == line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_hardcoded_address_in_initialize() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn initialize(env: Env) {
                    let admin = "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
                    env.storage().instance().set(&"admin", &admin);
                }
            }
        "#;

        let findings = InitHardcodedAdminRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, INIT_HARDCODED_ADMIN);
        assert!(findings[0].location.contains("initialize"));
    }

    #[test]
    fn detects_hardcoded_bytes_in_init_without_admin_param() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn init(env: Env) {
                    let admin_bytes = b"01234567890123456789012345678901";
                    env.storage().instance().set(&"admin", &admin_bytes);
                }
            }
        "#;

        let findings = InitHardcodedAdminRule::new().check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, INIT_HARDCODED_ADMIN);
    }

    #[test]
    fn allows_init_with_formal_admin_parameter() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn initialize(env: Env, admin: Address) {
                    admin.require_auth();
                    env.storage().instance().set(&"admin", &admin);
                }
            }
        "#;

        let findings = InitHardcodedAdminRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn respects_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn initialize(env: Env) {
                    // sanctifier:ignore[SANCT_INIT_HARDCODED_ADMIN]
                    let admin = "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
                    env.storage().instance().set(&"admin", &admin);
                }
            }
        "#;

        let findings = InitHardcodedAdminRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
