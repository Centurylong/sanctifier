use sanctifier_core::rules::{hardcoded_decimals::HardcodedDecimalsRule, Rule};

#[test]
fn snapshot_hardcoded_decimals() {
    let findings = HardcodedDecimalsRule::new().check(include_str!(
        "fixtures/detectors/hardcoded_decimals.rs"
    ));
    insta::assert_yaml_snapshot!("hardcoded_decimals", findings);
}
