use crate::finding_codes::SANCT_VIEW_PANIC;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Flags reachable panics (`panic!`, `unwrap`, `expect`, `unreachable!`, raw
/// indexing) inside `#[contractimpl]` view/getter entrypoints.
///
/// View functions are assumed by off-chain callers (indexers, dashboards,
/// other contracts doing a read-only call) to be safe to invoke. A panic in
/// one of these aborts the whole read, which is a much worse failure mode
/// than a state-mutating entrypoint reverting.
pub struct ViewPanicRule;

impl ViewPanicRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ViewPanicRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ViewPanicRule {
    fn name(&self) -> &str {
        "view_panic"
    }

    fn description(&self) -> &str {
        "Detects reachable panics inside view/getter contract entrypoints"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractViewVisitor {
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

struct ContractViewVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ContractViewVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ContractViewVisitor {
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

                if !is_view_function(&function.sig) {
                    continue;
                }

                let mut function_visitor = ViewFunctionVisitor {
                    fn_name: function.sig.ident.to_string(),
                    issues: Vec::new(),
                };
                function_visitor.visit_block(&function.block);

                self.violations.extend(
                    function_visitor
                        .issues
                        .into_iter()
                        .filter(|issue| !is_suppressed(&self.suppressions, issue.line))
                        .map(PanicIssue::into_violation),
                );
            }
        }
    }
}

struct ViewFunctionVisitor {
    fn_name: String,
    issues: Vec<PanicIssue>,
}

impl<'ast> Visit<'ast> for ViewFunctionVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if method == "unwrap" || method == "expect" {
            self.issues.push(PanicIssue {
                fn_name: self.fn_name.clone(),
                kind: method,
                line: node.method.span().start().line,
            });
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
        if let Some(kind) = panicking_macro(&node.mac) {
            self.issues.push(PanicIssue {
                fn_name: self.fn_name.clone(),
                kind,
                line: node.mac.path.segments[0].ident.span().start().line,
            });
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_stmt_macro(&mut self, node: &'ast syn::StmtMacro) {
        if let Some(kind) = panicking_macro(&node.mac) {
            self.issues.push(PanicIssue {
                fn_name: self.fn_name.clone(),
                kind,
                line: node.mac.path.segments[0].ident.span().start().line,
            });
        }
        syn::visit::visit_stmt_macro(self, node);
    }

    fn visit_expr_index(&mut self, node: &'ast syn::ExprIndex) {
        self.issues.push(PanicIssue {
            fn_name: self.fn_name.clone(),
            kind: "indexing".to_string(),
            line: node.bracket_token.span.join().start().line,
        });
        syn::visit::visit_expr_index(self, node);
    }
}

struct PanicIssue {
    fn_name: String,
    kind: String,
    line: usize,
}

impl PanicIssue {
    fn into_violation(self) -> RuleViolation {
        let (message, suggestion) = match self.kind.as_str() {
            "indexing" => (
                format!(
                    "{SANCT_VIEW_PANIC}: raw indexing in view entrypoint `{}` can abort the read on an out-of-bounds access",
                    self.fn_name
                ),
                "Use `.get(idx)` and return `Option`/`Result` instead of indexing directly"
                    .to_string(),
            ),
            "panic!" | "unreachable!" => (
                format!(
                    "{SANCT_VIEW_PANIC}: `{}` in view entrypoint `{}` aborts callers that assume reads are safe",
                    self.kind, self.fn_name
                ),
                "Return an `Option`/`Result` describing the missing/invalid state instead of panicking"
                    .to_string(),
            ),
            _ => (
                format!(
                    "{SANCT_VIEW_PANIC}: `{}` in view entrypoint `{}` can abort the read for indexers and dashboards",
                    self.kind, self.fn_name
                ),
                format!(
                    "Replace `.{}()` with typed `Option`/`Result` handling in this getter",
                    self.kind
                ),
            ),
        };

        RuleViolation::new(
            SANCT_VIEW_PANIC,
            Severity::Warning,
            message,
            format!("{}:{}", self.fn_name, self.line),
        )
        .with_suggestion(suggestion)
    }
}

/// A function is treated as a view/getter if it takes `&self`-style read-only
/// access (no mutation is implied by the signature) and either its name uses a
/// common read-only prefix/suffix or it returns a value without taking any
/// owned, non-`Env` argument that looks like a command payload.
fn is_view_function(sig: &syn::Signature) -> bool {
    if matches!(sig.output, syn::ReturnType::Default) {
        return false;
    }

    let name = sig.ident.to_string();
    const GETTER_PREFIXES: &[&str] = &["get_", "is_", "has_", "query_", "view_", "read_"];
    const GETTER_EXACT: &[&str] = &["balance", "supply", "owner", "admin"];
    const GETTER_SUFFIXES: &[&str] = &["_of"];

    GETTER_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
        || GETTER_EXACT.contains(&name.as_str())
        || GETTER_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn panicking_macro(mac: &syn::Macro) -> Option<String> {
    let ident = mac.path.segments.last()?.ident.to_string();
    matches!(ident.as_str(), "panic" | "unreachable").then(|| format!("{ident}!"))
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
            line.contains("sanctifier:ignore[SANCT_VIEW_PANIC]")
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
    fn detects_unwrap_in_getter_entrypoint() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env, Symbol};

            #[contractimpl]
            impl Contract {
                pub fn get_price(env: Env, asset: Symbol) -> i128 {
                    env.storage().persistent().get(&asset).unwrap()
                }
            }
        "#;

        let findings = ViewPanicRule::new().check(source);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule_name, SANCT_VIEW_PANIC);
        assert!(findings[0].location.contains("get_price"));
    }

    #[test]
    fn ignores_unwrap_in_non_getter_mutating_entrypoint() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env, Symbol};

            #[contractimpl]
            impl Contract {
                pub fn set_price(env: Env, asset: Symbol, price: i128) {
                    env.storage().persistent().set(&asset, &price);
                    env.storage().persistent().get(&asset).unwrap()
                }
            }
        "#;

        let findings = ViewPanicRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn detects_panic_macro_and_raw_indexing_in_getter() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env, Vec};

            #[contractimpl]
            impl Contract {
                pub fn get_holder(env: Env, holders: Vec<u64>, idx: u32) -> u64 {
                    if idx > 1000 {
                        panic!("index too large");
                    }
                    holders[idx]
                }
            }
        "#;

        let findings = ViewPanicRule::new().check(source);

        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings.iter().all(|f| f.rule_name == SANCT_VIEW_PANIC));
    }

    #[test]
    fn skips_cfg_test_modules_and_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env, Symbol};

            #[contractimpl]
            impl Contract {
                pub fn get_admin(env: Env) -> Symbol {
                    // sanctifier:ignore[SANCT_VIEW_PANIC]
                    env.storage().instance().get(&"admin").unwrap()
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[contractimpl]
                impl Contract {
                    pub fn get_test_admin(env: Env) -> Symbol {
                        env.storage().instance().get(&"admin").unwrap()
                    }
                }
            }
        "#;

        let findings = ViewPanicRule::new().check(source);

        assert!(findings.is_empty(), "{findings:#?}");
    }
}
