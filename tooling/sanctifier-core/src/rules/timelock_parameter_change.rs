use crate::finding_codes::TIMELOCK_PARAMETER_CHANGE;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Advisory detector for instant changes to critical governance parameters.
pub struct TimelockParameterChangeRule;

impl TimelockParameterChangeRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TimelockParameterChangeRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for TimelockParameterChangeRule {
    fn name(&self) -> &str {
        "timelock_parameter_change"
    }

    fn description(&self) -> &str {
        "Detects critical parameter setters that apply immediately without a timelock"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = TimelockVisitor {
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

struct TimelockVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl TimelockVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for TimelockVisitor {
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

fn check_function(function: &syn::ImplItemFn) -> Option<TimelockIssue> {
    let fn_name = function.sig.ident.to_string();
    let body = quote::quote!(#function.block).to_string();
    let compact_body = compact_lower(&body);
    let compact_name = compact_lower(&fn_name);

    if !is_critical_setter_name(&compact_name) || !writes_critical_parameter(&compact_body) {
        return None;
    }

    if has_timelock_pattern(&compact_name) || has_timelock_pattern(&compact_body) {
        return None;
    }

    Some(TimelockIssue {
        fn_name,
        parameter: critical_keyword(&compact_body)
            .or_else(|| critical_keyword(&compact_name))
            .unwrap_or("critical parameter")
            .to_string(),
        line: function.sig.ident.span().start().line,
    })
}

struct TimelockIssue {
    fn_name: String,
    parameter: String,
    line: usize,
}

impl TimelockIssue {
    fn into_violation(self) -> RuleViolation {
        RuleViolation::new(
            TIMELOCK_PARAMETER_CHANGE,
            Severity::Info,
            format!(
                "{TIMELOCK_PARAMETER_CHANGE}: `{}` updates `{}` without a visible timelock",
                self.fn_name, self.parameter
            ),
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(
            "Route critical parameter changes through a scheduled pending value with a delay and an execute step"
                .to_string(),
        )
    }
}

fn is_critical_setter_name(name: &str) -> bool {
    let setter = ["change", "configure", "set", "update", "upgrade"]
        .iter()
        .any(|term| name.contains(term));

    setter && critical_keyword(name).is_some()
}

fn writes_critical_parameter(body: &str) -> bool {
    body.contains(".set(") && critical_keyword(body).is_some()
}

fn critical_keyword(value: &str) -> Option<&'static str> {
    [
        "admin", "cap", "fee", "limit", "manager", "oracle", "rate", "reserve", "treasury",
        "upgrade",
    ]
    .into_iter()
    .find(|keyword| value.contains(keyword))
}

fn has_timelock_pattern(value: &str) -> bool {
    [
        "delay",
        "eta",
        "executeafter",
        "pending",
        "proposal",
        "queue",
        "schedule",
        "timelock",
    ]
    .iter()
    .any(|keyword| value.contains(keyword))
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
            line.contains("sanctifier:ignore[SANCT_TIMELOCK_PARAMETER_CHANGE]")
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
    fn detects_instant_critical_parameter_updates() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn set_fee_rate(env: Env, fee_rate: u32) {
                    env.storage().instance().set(&"fee_rate", &fee_rate);
                }

                pub fn update_oracle(env: Env, oracle: Address) {
                    env.storage().instance().set(&"oracle", &oracle);
                }
            }
        "#;

        let findings = TimelockParameterChangeRule::new().check(source);

        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|finding| finding.rule_name == TIMELOCK_PARAMETER_CHANGE));
    }

    #[test]
    fn allows_scheduled_pending_changes_and_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env};

            #[contractimpl]
            impl Contract {
                pub fn schedule_fee_rate(env: Env, fee_rate: u32, eta: u64) {
                    env.storage().instance().set(&"pending_fee_rate", &fee_rate);
                    env.storage().instance().set(&"fee_rate_eta", &eta);
                }

                pub fn execute_fee_rate(env: Env) {
                    let pending_fee_rate: u32 = env.storage().instance().get(&"pending_fee_rate").unwrap();
                    env.storage().instance().set(&"fee_rate", &pending_fee_rate);
                }

                // sanctifier:ignore[SANCT_TIMELOCK_PARAMETER_CHANGE]
                pub fn set_fee_rate(env: Env, fee_rate: u32) {
                    env.storage().instance().set(&"fee_rate", &fee_rate);
                }
            }
        "#;

        let findings = TimelockParameterChangeRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
