use crate::finding_codes::BLS_SUBGROUP_UNCHECKED;
use crate::rules::{Rule, RuleViolation, Severity};
use quote::ToTokens;
use syn::visit::Visit;
use syn::{parse_str, Attribute, File};

/// Flags BLS12-381 pairing usage where a proof point reaches the pairing call
/// without a subgroup or curve-membership check.
///
/// The pairing `e: G1 × G2 → GT` is only defined on the prime-order subgroups.
/// Both BLS12-381 curve groups have a cofactor, so the full curve contains
/// points of small order outside the subgroup the proof system reasons about.
/// A verifier that pairs an unchecked point is evaluating something outside the
/// domain its soundness proof covers, and the two classic consequences are:
///
/// * **Malleability.** Adding a torsion point to a valid proof point yields a
///   different serialization that still passes verification, so "this proof was
///   already used" checks keyed on the proof bytes can be bypassed.
/// * **Forgery.** With the pairing evaluated off-subgroup, the algebraic
///   relations the verifier is checking no longer pin down the witness, and
///   proofs can be produced without one.
///
/// The detector reports two shapes, both of which put an unvalidated point in
/// front of a pairing:
///
/// 1. A point built with an `_unchecked` constructor or deserializer. In
///    arkworks these are the ones that deliberately skip the subgroup check;
///    they are safe **only** when followed by an explicit check, which is the
///    idiom this rule looks for.
/// 2. A point arriving as a function parameter already typed as `G1Affine` /
///    `G2Affine` / `G1Projective` / `G2Projective`. Deserialization happened
///    elsewhere, so nothing in this function establishes membership.
///
/// It stays quiet whenever a membership check is present, and only fires in
/// files that actually use BLS12-381.
pub struct BlsSubgroupCheckRule;

impl BlsSubgroupCheckRule {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BlsSubgroupCheckRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for BlsSubgroupCheckRule {
    fn name(&self) -> &str {
        "bls_subgroup_check"
    }

    fn description(&self) -> &str {
        "Detects BLS12-381 pairing usage where proof points reach the pairing without a subgroup/curve membership check"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        // Cheap gate first: without BLS12-381 in the file there is no pairing
        // domain to leave, and an `_unchecked` constructor means something else
        // entirely.
        if !uses_bls12_381(source) {
            return Vec::new();
        }

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

const BLS_HINTS: &[&str] = &[
    "bls12_381",
    "bls12381",
    "Bls12_381",
    "G1Affine",
    "G2Affine",
    "G1Projective",
    "G2Projective",
];

const POINT_TYPE_HINTS: &[&str] = &["G1Affine", "G2Affine", "G1Projective", "G2Projective"];

/// Calls that establish membership. `clear_cofactor` / `mul_by_cofactor` are
/// included because mapping into the subgroup is an equally valid remedy to
/// rejecting.
const MEMBERSHIP_CHECKS: &[&str] = &[
    "is_in_correct_subgroup_assuming_on_curve",
    "is_in_correct_subgroup",
    "is_in_subgroup",
    "is_torsion_free",
    "is_on_curve",
    "subgroup_check",
    "check_subgroup",
    "clear_cofactor",
    "mul_by_cofactor",
    "into_subgroup",
    "validate_point",
    "validate_g1",
    "validate_g2",
];

const PAIRING_HINTS: &[&str] = &["pairing", "verify", "miller_loop", "final_exponentiation"];

fn uses_bls12_381(source: &str) -> bool {
    BLS_HINTS.iter().any(|hint| source.contains(hint))
}

fn is_unchecked_point_constructor(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.ends_with("_unchecked")
        && (lower.contains("from")
            || lower.contains("deserialize")
            || lower.contains("new")
            || lower.contains("read"))
}

fn is_membership_check(name: &str) -> bool {
    MEMBERSHIP_CHECKS.iter().any(|needle| name == *needle)
}

fn is_pairing_call(name: &str) -> bool {
    let lower = name.to_lowercase();
    PAIRING_HINTS.iter().any(|hint| lower.contains(hint))
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

    fn inspect_fn(&mut self, fn_name: String, sig: &syn::Signature, block: &syn::Block) {
        let mut body = BodyVisitor {
            unchecked_constructions: Vec::new(),
            membership_checked: false,
            pairing_calls: Vec::new(),
        };
        body.visit_block(block);

        let Some((pairing_kind, pairing_line)) = body.pairing_calls.first().cloned() else {
            return;
        };
        if body.membership_checked {
            return;
        }

        // Shape 1: an _unchecked constructor with no follow-up check.
        for (call, line) in &body.unchecked_constructions {
            if is_suppressed(&self.suppressions, *line) {
                continue;
            }
            self.violations.push(
                RuleViolation::new(
                    BLS_SUBGROUP_UNCHECKED,
                    Severity::Error,
                    format!(
                        "{BLS_SUBGROUP_UNCHECKED}: `{call}` in `{fn_name}` builds a BLS12-381 point that reaches `{pairing_kind}` with no subgroup check"
                    ),
                    format!("{fn_name}:{line}"),
                )
                .with_suggestion(
                    "Call `is_in_correct_subgroup_assuming_on_curve()` (and `is_on_curve()`) on the point and reject on failure, or use the checked deserializer. An `_unchecked` constructor is safe only when the check it skipped is performed explicitly afterwards".to_string(),
                ),
            );
        }

        if !body.unchecked_constructions.is_empty() {
            return;
        }

        // Shape 2: a point handed in already typed, so deserialization — and
        // any check that went with it — happened somewhere this function
        // cannot see.
        let point_params: Vec<String> = sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                syn::FnArg::Typed(pat_type) => {
                    let type_text = pat_type.ty.to_token_stream().to_string();
                    if !POINT_TYPE_HINTS.iter().any(|hint| type_text.contains(hint)) {
                        return None;
                    }
                    match pat_type.pat.as_ref() {
                        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
                        _ => None,
                    }
                }
                _ => None,
            })
            .collect();

        if point_params.is_empty() || is_suppressed(&self.suppressions, pairing_line) {
            return;
        }

        let param_list = point_params.join(", ");
        self.violations.push(
            RuleViolation::new(
                BLS_SUBGROUP_UNCHECKED,
                Severity::Error,
                format!(
                    "{BLS_SUBGROUP_UNCHECKED}: `{pairing_kind}` in `{fn_name}` pairs `{param_list}` without checking subgroup membership"
                ),
                format!("{fn_name}:{pairing_line}"),
            )
            .with_suggestion(format!(
                "Check `{param_list}` with `is_on_curve()` and `is_in_correct_subgroup_assuming_on_curve()` before pairing. The pairing is only defined on the prime-order subgroup, and BLS12-381 has a cofactor in both G1 and G2, so an off-subgroup point makes the verification equation prove nothing"
            )),
        );
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

    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if !self.in_test_module() && !has_cfg_test(&node.attrs) {
            self.inspect_fn(node.sig.ident.to_string(), &node.sig, &node.block);
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if !self.in_test_module() {
            self.inspect_fn(node.sig.ident.to_string(), &node.sig, &node.block);
        }
        syn::visit::visit_impl_item_fn(self, node);
    }
}

struct BodyVisitor {
    unchecked_constructions: Vec<(String, usize)>,
    membership_checked: bool,
    pairing_calls: Vec<(String, usize)>,
}

impl BodyVisitor {
    fn note(&mut self, name: &str, line: usize) {
        if is_membership_check(name) {
            self.membership_checked = true;
        }
        if is_unchecked_point_constructor(name) {
            self.unchecked_constructions.push((name.to_string(), line));
        }
        if is_pairing_call(name) {
            self.pairing_calls.push((name.to_string(), line));
        }
    }
}

impl<'ast> Visit<'ast> for BodyVisitor {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let name = node.method.to_string();
        self.note(&name, node.method.span().start().line);
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(expr_path) = node.func.as_ref() {
            if let Some(last) = expr_path.path.segments.last() {
                let name = last.ident.to_string();
                self.note(&name, last.ident.span().start().line);
            }
        }
        syn::visit::visit_expr_call(self, node);
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
            line.contains("sanctifier:ignore[SANCT_BLS_SUBGROUP_UNCHECKED]")
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
    fn flags_unchecked_deserialization_reaching_a_pairing() {
        let source = r#"
            use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};

            fn verify_proof(proof_bytes: &[u8], vk: &VerifyingKey) -> bool {
                let a = G1Affine::deserialize_uncompressed_unchecked(proof_bytes).unwrap();
                let b = G2Affine::deserialize_uncompressed_unchecked(proof_bytes).unwrap();
                Bls12_381::pairing(a, b) == vk.alpha_beta
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert_eq!(findings.len(), 2, "{findings:#?}");
        assert!(findings
            .iter()
            .all(|f| f.rule_name == BLS_SUBGROUP_UNCHECKED && f.severity == Severity::Error));
    }

    #[test]
    fn accepts_unchecked_deserialization_followed_by_an_explicit_check() {
        let source = r#"
            use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};

            fn verify_proof(proof_bytes: &[u8], vk: &VerifyingKey) -> bool {
                let a = G1Affine::deserialize_uncompressed_unchecked(proof_bytes).unwrap();
                if !a.is_on_curve() || !a.is_in_correct_subgroup_assuming_on_curve() {
                    return false;
                }
                Bls12_381::pairing(a, vk.g2) == vk.alpha_beta
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn flags_a_caller_supplied_point_paired_without_a_check() {
        let source = r#"
            use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};

            fn verify_proof(a: G1Affine, b: G2Affine, vk: &VerifyingKey) -> bool {
                Bls12_381::pairing(a, b) == vk.alpha_beta
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert_eq!(findings.len(), 1, "{findings:#?}");
        assert!(findings[0].message.contains("a, b"), "{findings:#?}");
    }

    #[test]
    fn accepts_a_caller_supplied_point_that_is_checked() {
        let source = r#"
            use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};

            fn verify_proof(a: G1Affine, b: G2Affine, vk: &VerifyingKey) -> bool {
                if !a.is_in_correct_subgroup_assuming_on_curve() {
                    return false;
                }
                Bls12_381::pairing(a, b) == vk.alpha_beta
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_files_that_do_not_use_bls12_381() {
        let source = r#"
            fn load(bytes: &[u8]) -> Config {
                Config::from_bytes_unchecked(bytes)
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn ignores_bls_code_that_never_pairs() {
        let source = r#"
            use ark_bls12_381::G1Affine;

            fn store(bytes: &[u8]) -> G1Affine {
                G1Affine::deserialize_uncompressed_unchecked(bytes).unwrap()
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn skips_test_modules_and_honours_inline_suppression() {
        let source = r#"
            use ark_bls12_381::{Bls12_381, G1Affine, G2Affine};

            fn verify_proof(proof_bytes: &[u8], vk: &VerifyingKey) -> bool {
                // sanctifier:ignore[SANCT_BLS_SUBGROUP_UNCHECKED]
                let a = G1Affine::deserialize_uncompressed_unchecked(proof_bytes).unwrap();
                Bls12_381::pairing(a, vk.g2) == vk.alpha_beta
            }

            #[cfg(test)]
            mod tests {
                use super::*;

                fn verify_in_test(a: G1Affine, b: G2Affine, vk: &VerifyingKey) -> bool {
                    Bls12_381::pairing(a, b) == vk.alpha_beta
                }
            }
        "#;

        let findings = BlsSubgroupCheckRule::new().check(source);
        assert!(findings.is_empty(), "{findings:#?}");
    }
}
