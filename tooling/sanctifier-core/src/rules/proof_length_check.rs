use crate::finding_codes::PROOF_LENGTH_UNVALIDATED;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Flags `#[contractimpl]` entrypoints that pass a proof or public-input
/// byte array into a verifier call (`.verify(...)`, `verify_with_processed_vk`,
/// `Groth16::verify`, etc.) without validating that array's length anywhere
/// in the function first.
///
/// Soroban verifier contracts commonly deserialize a fixed-size buffer from a
/// caller-supplied `Bytes`/`Vec<u8>` argument (see
/// `contracts/zk-verifier/src/lib.rs`). Skipping the length check before that
/// deserialization either panics on a truncated/oversized proof or, worse,
/// silently truncates attacker-controlled input into a still-"valid" shape
/// that bypasses the intended check.
pub struct ProofLengthCheckRule;

impl ProofLengthCheckRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProofLengthCheckRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for ProofLengthCheckRule {
    fn name(&self) -> &str {
        "proof_length_check"
    }

    fn description(&self) -> &str {
        "Detects verify() calls on proof/public-input byte arrays with no length validation beforehand"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = ContractVisitor {
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

const PROOF_LIKE_NAMES: &[&str] = &["proof", "public_input", "public_inputs", "inputs"];

fn is_proof_like_ident(name: &str) -> bool {
    let lower = name.to_lowercase();
    PROOF_LIKE_NAMES
        .iter()
        .any(|needle| lower == *needle || lower.starts_with(&format!("{needle}_")))
}

struct ContractVisitor {
    violations: Vec<RuleViolation>,
    suppressions: Vec<usize>,
    test_depth: usize,
}

impl ContractVisitor {
    fn in_test_module(&self) -> bool {
        self.test_depth > 0
    }
}

impl<'ast> Visit<'ast> for ContractVisitor {
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

                let proof_params: Vec<String> = function
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                            syn::Pat::Ident(pat_ident) => {
                                let name = pat_ident.ident.to_string();
                                is_proof_like_ident(&name).then_some(name)
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();

                if proof_params.is_empty() {
                    continue;
                }

                let mut function_visitor = VerifyCallVisitor {
                    fn_name: function.sig.ident.to_string(),
                    proof_params: proof_params.clone(),
                    validated_params: std::collections::HashSet::new(),
                    verify_calls: Vec::new(),
                };
                function_visitor.visit_block(&function.block);

                if function_visitor.verify_calls.is_empty() {
                    continue;
                }

                let unvalidated: Vec<&String> = proof_params
                    .iter()
                    .filter(|p| !function_visitor.validated_params.contains(*p))
                    .collect();

                if unvalidated.is_empty() {
                    continue;
                }

                for (verify_kind, line) in &function_visitor.verify_calls {
                    if is_suppressed(&self.suppressions, *line) {
                        continue;
                    }
                    let param_list = unvalidated
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    self.violations.push(
                        RuleViolation::new(
                            PROOF_LENGTH_UNVALIDATED,
                            Severity::Warning,
                            format!(
                                "{PROOF_LENGTH_UNVALIDATED}: `{}` in `{}` is reached without validating the length of `{}`",
                                verify_kind, function_visitor.fn_name, param_list
                            ),
                            format!("{}:{}", function_visitor.fn_name, line),
                        )
                        .with_suggestion(format!(
                            "Check `{param_list}.len()` against the expected size before calling the verifier, so a truncated or oversized proof/input is rejected instead of panicking or deserializing into an unintended value"
                        )),
                    );
                }
            }
        }

        syn::visit::visit_item_impl(self, node);
    }
}

struct VerifyCallVisitor {
    fn_name: String,
    proof_params: Vec<String>,
    validated_params: std::collections::HashSet<String>,
    verify_calls: Vec<(String, usize)>,
}

impl<'ast> Visit<'ast> for VerifyCallVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if method == "len" {
            if let syn::Expr::Path(expr_path) = node.receiver.as_ref() {
                if let Some(ident) = expr_path.path.get_ident() {
                    let name = ident.to_string();
                    if self.proof_params.iter().any(|p| p == &name) {
                        self.validated_params.insert(name);
                    }
                }
            }
        }

        if method.contains("verify") {
            self.verify_calls
                .push((method.clone(), node.method.span().start().line));
        }

        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = node.func.as_ref() {
            if let Some(last) = expr_path.path.segments.last() {
                let name = last.ident.to_string();
                if name.to_lowercase().contains("verify") {
                    self.verify_calls
                        .push((name, last.ident.span().start().line));
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
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
            line.contains("sanctifier:ignore[SANCT_PROOF_LENGTH_UNVALIDATED]")
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
    fn detects_verify_without_length_check() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env};

            #[contractimpl]
            impl Contract {
                pub fn verify(env: Env, proof_bytes: Bytes, public_inputs_bytes: Bytes) -> bool {
                    let mut slice = [0u8; 512];
                    proof_bytes.copy_into_slice(&mut slice);
                    inner_verify(&slice)
                }
            }

            fn inner_verify(_p: &[u8]) -> bool {
                true
            }
        "#;

        let findings = ProofLengthCheckRule::new().check(source);
        assert!(!findings.is_empty(), "{findings:#?}");
        assert!(findings
            .iter()
            .all(|f| f.rule_name == PROOF_LENGTH_UNVALIDATED));
    }

    #[test]
    fn ignores_verify_with_prior_length_check() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env};

            #[contractimpl]
            impl Contract {
                pub fn verify(env: Env, proof_bytes: Bytes, public_inputs_bytes: Bytes) -> bool {
                    if proof_bytes.len() != 512 || public_inputs_bytes.len() != 128 {
                        return false;
                    }
                    inner_verify(&proof_bytes)
                }
            }

            fn inner_verify(_p: &Bytes) -> bool {
                true
            }
        "#;

        let findings = ProofLengthCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_functions_without_verify_call() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env};

            #[contractimpl]
            impl Contract {
                pub fn store(env: Env, proof_bytes: Bytes) {
                    env.storage().instance().set(&0u32, &proof_bytes);
                }
            }
        "#;

        let findings = ProofLengthCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn skips_cfg_test_modules_and_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env};

            #[contractimpl]
            impl Contract {
                pub fn verify(env: Env, proof_bytes: Bytes) -> bool {
                    // sanctifier:ignore[SANCT_PROOF_LENGTH_UNVALIDATED]
                    inner_verify(&proof_bytes)
                }
            }

            fn inner_verify(_p: &Bytes) -> bool {
                true
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[contractimpl]
                impl Contract {
                    pub fn verify_test(env: Env, proof_bytes: Bytes) -> bool {
                        inner_verify(&proof_bytes)
                    }
                }
            }
        "#;

        let findings = ProofLengthCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
