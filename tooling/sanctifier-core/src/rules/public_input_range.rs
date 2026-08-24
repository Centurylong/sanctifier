use crate::finding_codes::PUBLIC_INPUT_UNVALIDATED;
use crate::rules::{Rule, RuleViolation, Severity};
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Flags `#[contractimpl]` entrypoints that feed caller-supplied public inputs
/// into a proof verification without first checking that each one is a
/// canonical field element in range.
///
/// A Groth16/PLONK verifier's soundness argument assumes every public input is
/// a canonical element of the scalar field `F_r`. A caller who can hand the
/// verifier a value `>= r`, or a non-canonical encoding of a value `< r`, gets
/// two things the circuit author did not intend:
///
/// * **Aliasing.** `x` and `x + r` reduce to the same field element, so an
///   input the contract's own business logic reads as one number (a large
///   amount, a different account id) is the number the pairing check actually
///   validates. The proof verifies; the contract acted on something else.
/// * **Implementation-defined reduction.** Whether a backend rejects, wraps, or
///   silently truncates an out-of-range limb is not part of any proof system's
///   security proof, and differs between arkworks, bellman and hand-rolled
///   assembly. A verifier that leans on it is relying on undefined behaviour.
///
/// This is the check that distinguishes "we call `verify` correctly" from "our
/// verifier is sound", and it is routinely missing from integration code
/// because the happy path passes either way.
///
/// The detector looks for a range or canonicality check reaching each
/// public-input parameter *before* the verification call, and stays quiet when
/// one is present — including the common forms: comparison against a modulus
/// constant, a checked deserializer that returns `Option`/`Result`, or a
/// dedicated validation helper.
pub struct PublicInputRangeRule;

impl PublicInputRangeRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PublicInputRangeRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for PublicInputRangeRule {
    fn name(&self) -> &str {
        "public_input_range"
    }

    fn description(&self) -> &str {
        "Detects proof verification that consumes public inputs without validating they are canonical field elements in range"
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

/// Parameter names that carry public inputs into a verifier. Deliberately does
/// not include bare `proof`: a proof is not a public input, and flagging it
/// here would duplicate `proof_length_check` on a different axis.
const PUBLIC_INPUT_NAMES: &[&str] = &[
    "public_input",
    "public_inputs",
    "pub_input",
    "pub_inputs",
    "public_signal",
    "public_signals",
    "pub_signals",
    "inputs",
    "instance",
    "instances",
];

/// Names of constants that hold a field modulus or group order. A comparison
/// against one of these is the canonical hand-rolled range check.
const MODULUS_HINTS: &[&str] = &[
    "modulus",
    "field_order",
    "field_modulus",
    "scalar_field",
    "scalar_modulus",
    "group_order",
    "subgroup_order",
    "curve_order",
    "fr_modulus",
    "bls12_381_r",
    "bn254_r",
];

/// Calls that perform the check for you by construction: they either return
/// `Option`/`Result` on a non-canonical encoding, or answer the question
/// directly.
const VALIDATING_CALLS: &[&str] = &[
    "from_canonical_bytes",
    "from_bytes_checked",
    "from_repr",
    "from_repr_vartime",
    "deserialize_compressed",
    "deserialize_uncompressed",
    "is_canonical",
    "is_in_field",
    "is_valid_field_element",
    "is_less_than_modulus",
    "check_canonical",
    "validate_public_input",
    "validate_public_inputs",
    "validate_field_element",
    "validate_field_elements",
    "to_field_element",
    "try_into_field",
];

const VERIFY_HINTS: &[&str] = &["verify", "pairing", "check_proof"];

fn is_public_input_ident(name: &str) -> bool {
    let lower = name.to_lowercase();
    PUBLIC_INPUT_NAMES
        .iter()
        .any(|needle| lower == *needle || lower.starts_with(&format!("{needle}_")))
}

fn mentions_modulus(text: &str) -> bool {
    let lower = text.to_lowercase();
    MODULUS_HINTS.iter().any(|hint| lower.contains(hint))
}

fn is_validating_call(name: &str) -> bool {
    let lower = name.to_lowercase();
    VALIDATING_CALLS.iter().any(|needle| lower == *needle)
        || (lower.contains("canonical") && !lower.contains("unchecked"))
        || (lower.contains("range") && lower.contains("check"))
}

fn is_verify_call(name: &str) -> bool {
    let lower = name.to_lowercase();
    VERIFY_HINTS.iter().any(|hint| lower.contains(hint))
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

                let input_params: Vec<String> = function
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| match arg {
                        syn::FnArg::Typed(pat_type) => match pat_type.pat.as_ref() {
                            syn::Pat::Ident(pat_ident) => {
                                let name = pat_ident.ident.to_string();
                                is_public_input_ident(&name).then_some(name)
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();

                if input_params.is_empty() {
                    continue;
                }

                let mut body = BodyVisitor {
                    validated: false,
                    verify_calls: Vec::new(),
                };
                body.visit_block(&function.block);

                // Nothing is verified here, so there is no soundness claim to
                // undermine — a plain setter that happens to take public inputs
                // is not this detector's business.
                let Some((verify_kind, verify_line)) = body.verify_calls.first().cloned() else {
                    continue;
                };

                if body.validated {
                    continue;
                }

                if is_suppressed(&self.suppressions, verify_line) {
                    continue;
                }

                let fn_name = function.sig.ident.to_string();
                let param_list = input_params.join(", ");
                self.violations.push(
                    RuleViolation::new(
                        PUBLIC_INPUT_UNVALIDATED,
                        Severity::Error,
                        format!(
                            "{PUBLIC_INPUT_UNVALIDATED}: `{verify_kind}` in `{fn_name}` verifies against `{param_list}` without checking they are canonical field elements in range"
                        ),
                        format!("{fn_name}:{verify_line}"),
                    )
                    .with_suggestion(format!(
                        "Reject `{param_list}` before verifying unless every element is canonically encoded and strictly less than the scalar field modulus r. A value >= r aliases to `value - r`, so the pairing check passes for a different number than the contract's own logic read"
                    )),
                );
            }
        }

        syn::visit::visit_item_impl(self, node);
    }
}

struct BodyVisitor {
    validated: bool,
    verify_calls: Vec<(String, usize)>,
}

impl BodyVisitor {
    /// A comparison against a modulus-like constant, in either direction, is
    /// the hand-rolled form of the check.
    fn note_comparison(&mut self, node: &syn::ExprBinary) {
        use syn::BinOp::{Ge, Gt, Le, Lt};
        if !matches!(node.op, Lt(_) | Le(_) | Gt(_) | Ge(_)) {
            return;
        }
        let left = quote_text(&node.left);
        let right = quote_text(&node.right);
        if mentions_modulus(&left) || mentions_modulus(&right) {
            self.validated = true;
        }
    }
}

impl<'ast> Visit<'ast> for BodyVisitor {
    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        self.note_comparison(node);
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if is_validating_call(&method) {
            self.validated = true;
        }
        if is_verify_call(&method) {
            self.verify_calls
                .push((method.clone(), node.method.span().start().line));
        }

        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = node.func.as_ref() {
            if let Some(last) = expr_path.path.segments.last() {
                let name = last.ident.to_string();
                if is_validating_call(&name) {
                    self.validated = true;
                }
                if is_verify_call(&name) {
                    self.verify_calls
                        .push((name, last.ident.span().start().line));
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

/// Renders an expression back to text for the modulus-name scan. Cheaper and
/// more robust than pattern-matching every shape a constant reference can take
/// (`MODULUS`, `crate::consts::FR_MODULUS`, `Fr::MODULUS`, ...).
fn quote_text(expr: &syn::Expr) -> String {
    use quote::ToTokens;
    expr.to_token_stream().to_string()
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
            line.contains("sanctifier:ignore[SANCT_PUBLIC_INPUT_UNVALIDATED]")
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
    fn flags_verification_with_unvalidated_public_inputs() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env, Vec};

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes, public_inputs: Vec<u64>) -> bool {
                    let vk = load_vk(&env);
                    Groth16::verify(&vk, &public_inputs, &proof)
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert_eq!(findings[0].rule_name, PUBLIC_INPUT_UNVALIDATED);
        assert_eq!(findings[0].severity, Severity::Error);
    }

    #[test]
    fn accepts_an_explicit_comparison_against_the_modulus() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env, Vec};

            const FR_MODULUS: u256 = 0;

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes, public_inputs: Vec<u256>) -> bool {
                    for input in public_inputs.iter() {
                        if input >= FR_MODULUS {
                            return false;
                        }
                    }
                    Groth16::verify(&load_vk(&env), &public_inputs, &proof)
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn accepts_a_checked_deserializer() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env, Vec};

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes, public_inputs: Bytes) -> bool {
                    let field_elements = match Fr::from_canonical_bytes(&public_inputs) {
                        Some(elements) => elements,
                        None => return false,
                    };
                    Groth16::verify(&load_vk(&env), &field_elements, &proof)
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn accepts_a_dedicated_validation_helper() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env, Vec};

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes, pub_signals: Vec<u256>) -> bool {
                    if !validate_public_inputs(&pub_signals) {
                        return false;
                    }
                    Groth16::verify(&load_vk(&env), &pub_signals, &proof)
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_entrypoints_that_never_verify() {
        let source = r#"
            use soroban_sdk::{contractimpl, Env, Vec};

            #[contractimpl]
            impl Verifier {
                pub fn store_inputs(env: Env, public_inputs: Vec<u64>) {
                    env.storage().instance().set(&0u32, &public_inputs);
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_verification_without_public_input_parameters() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env};

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes) -> bool {
                    Groth16::verify(&load_vk(&env), &[], &proof)
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn skips_test_modules_and_honours_inline_suppression() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env, Vec};

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes, public_inputs: Vec<u64>) -> bool {
                    // sanctifier:ignore[SANCT_PUBLIC_INPUT_UNVALIDATED]
                    Groth16::verify(&load_vk(&env), &public_inputs, &proof)
                }
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                #[contractimpl]
                impl Verifier {
                    pub fn verify_in_test(env: Env, proof: Bytes, public_inputs: Vec<u64>) -> bool {
                        Groth16::verify(&load_vk(&env), &public_inputs, &proof)
                    }
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn does_not_treat_unchecked_conversions_as_validation() {
        let source = r#"
            use soroban_sdk::{contractimpl, Bytes, Env, Vec};

            #[contractimpl]
            impl Verifier {
                pub fn verify_claim(env: Env, proof: Bytes, public_inputs: Bytes) -> bool {
                    let elements = Fr::from_canonical_bytes_unchecked(&public_inputs);
                    Groth16::verify(&load_vk(&env), &elements, &proof)
                }
            }
        "#;

        let findings = PublicInputRangeRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
    }
}
