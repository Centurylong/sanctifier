use sanctifier_core::rules::error_repr_instability::{
    capture_error_repr_baseline, ErrorReprInstabilityRule,
};
use sanctifier_core::Rule;

#[test]
fn snapshot_error_repr_instability() {
    let baseline_source = r#"#[contracterror]
#[repr(u32)]
pub enum Error {
    NotFound = 1,
    InvalidInput,
    Unauthorized,
}
"#;
    let baseline = capture_error_repr_baseline(baseline_source, "contracts/error.rs");
    let rule = ErrorReprInstabilityRule::with_baseline("contracts/error.rs", &baseline);
    let findings = rule.check(include_str!("fixtures/detectors/error_repr_instability.rs"));

    insta::assert_yaml_snapshot!("error_repr_instability", findings);
}
