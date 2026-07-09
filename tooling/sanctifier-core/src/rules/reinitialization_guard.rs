use crate::finding_codes::REINITIALIZATION_GUARD;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Detects initialize entrypoints that write deployment-time state without an
/// already-initialized guard.
pub struct ReinitializationGuardRule;

impl ReinitializationGuardRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReinitializationGuardRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ReinitializationGuardRule {
    fn name(&self) -> &str {
        "reinitialization_guard"
    }

    fn description(&self) -> &str {
        "Detects initialize entrypoints missing an already-initialized guard"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = InitVisitor {
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

struct InitVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl InitVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for InitVisitor {
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

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if self.in_test_module() || !has_attr(&node.attrs, "contractimpl") {
            syn::visit::visit_item_impl(self, node);
            return;
        }

        for item in &node.items {
            if let syn::ImplItem::Fn(function) = item {
                if !matches!(function.vis, syn::Visibility::Public(_)) {
                    continue;
                }

                if let Some(issue) = check_function(function) {
                    if !is_suppressed(&self.suppressions, issue.line) {
                        self.violations.push(issue.into_violation());
                    }
                }
            }
        }
    }
}

fn check_function(function: &syn::ImplItemFn) -> Option<InitIssue> {
    let fn_name = function.sig.ident.to_string();
    if !is_initialize_function(&fn_name) {
        return None;
    }

    let body = quote::quote!(#function.block).to_string();
    if !writes_initial_state(&body) || has_initialized_guard(&body) {
        return None;
    }

    Some(InitIssue {
        fn_name,
        line: function.sig.ident.span().start().line,
    })
}

struct InitIssue {
    fn_name: String,
    line: usize,
}

impl InitIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            REINITIALIZATION_GUARD,
            Severity::Warning,
            format!(
                "{REINITIALIZATION_GUARD}: `{}` writes initialization state without an already-initialized guard",
                self.fn_name
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(
            "Check an initialized flag or existing admin/config storage key before writing deployment-time state"
                .to_string(),
        )
    }
}

fn is_initialize_function(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(lower.as_str(), "init" | "initialize")
        || lower.starts_with("init_")
        || lower.starts_with("initialize_")
}

fn writes_initial_state(body: &str) -> bool {
    let compact = compact_lower(body);
    compact.contains(".set(")
        && [
            "admin",
            "config",
            "initialized",
            "manager",
            "owner",
            "treasury",
        ]
        .iter()
        .any(|keyword| compact.contains(keyword))
}

fn has_initialized_guard(body: &str) -> bool {
    let compact = compact_lower(body);
    let checks_existing_storage = (compact.contains(".has(") || compact.contains("containskey("))
        && [
            "admin",
            "config",
            "initialized",
            "manager",
            "owner",
            "treasury",
        ]
        .iter()
        .any(|keyword| compact.contains(keyword));

    checks_existing_storage
        || compact.contains("alreadyinitialized")
        || compact.contains("isinitialized(")
        || compact.contains("isinitialized")
        || compact.contains("initialized==true")
        || compact.contains("initialized{")
        || compact.contains("initialized)")
}

fn compact_lower(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '_')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name)
    })
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
            line.contains("sanctifier:ignore[SANCT_REINITIALIZATION_GUARD]")
                .then_some(index + 1)
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
    fn detects_initialize_without_guard() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn initialize(env: Env, admin: Address) {
                    env.storage().instance().set(&"admin", &admin);
                    env.storage().instance().set(&"initialized", &true);
                }
            }
        "#;

        let findings = ReinitializationGuardRule::new().check(source);

        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, REINITIALIZATION_GUARD);
    }

    #[test]
    fn allows_initialized_guard_and_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Address, Env};

            #[contractimpl]
            impl Contract {
                pub fn initialize(env: Env, admin: Address) {
                    if env.storage().instance().has(&"admin") {
                        panic!("already initialized");
                    }
                    env.storage().instance().set(&"admin", &admin);
                }

                // sanctifier:ignore[SANCT_REINITIALIZATION_GUARD]
                pub fn init_for_test(env: Env, owner: Address) {
                    env.storage().instance().set(&"owner", &owner);
                }
            }
        "#;

        let findings = ReinitializationGuardRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
