//! Shared integration-test helpers for detector authors.
//!
//! Keep snapshot assertions in a macro so `insta` sees the invocation file as
//! the source. That preserves the existing `tests/snapshots/detector_snapshots__*.snap`
//! layout while removing the repeated test wrapper and finding collection.

macro_rules! rule_fixture_snapshot {
    ($test_name:ident, $snapshot_name:literal, $rule:expr, $fixture:expr $(,)?) => {
        #[test]
        fn $test_name() {
            let rule = $rule;
            let findings = sanctifier_core::rules::Rule::check(&rule, $fixture);
            insta::assert_yaml_snapshot!($snapshot_name, findings);
        }
    };
}

pub(crate) use rule_fixture_snapshot;
