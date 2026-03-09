use sanctifier_core::{Analyzer, SanctifyConfig};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct AnalysisReport<'a> {
    size_warnings: Vec<sanctifier_core::SizeWarning>,
    unsafe_patterns: Vec<sanctifier_core::UnsafePattern>,
    auth_gaps: Vec<String>,
    panic_issues: Vec<sanctifier_core::PanicIssue>,
    arithmetic_issues: Vec<sanctifier_core::ArithmeticIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_rule_matches: Option<Vec<sanctifier_core::CustomRuleMatch>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kani_metrics: Option<KaniMetrics<'a>>,
}

#[derive(Serialize)]
struct KaniMetrics<'a> {
    total_assertions: u32,
    proven: u32,
    failed: u32,
    unreachable: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    notes: Option<&'a str>,
}

#[wasm_bindgen]
pub fn analyze(source: &str) -> JsValue {
    let analyzer = Analyzer::new(SanctifyConfig::default());

    let size_warnings = analyzer.analyze_ledger_size(source);
    let unsafe_patterns = analyzer.analyze_unsafe_patterns(source);
    let auth_gaps = analyzer.scan_auth_gaps(source);
    let panic_issues = analyzer.scan_panics(source);
    let arithmetic_issues = analyzer.scan_arithmetic_overflow(source);

    let report = AnalysisReport {
        size_warnings,
        unsafe_patterns,
        auth_gaps,
        panic_issues,
        arithmetic_issues,
        custom_rule_matches: None,
        kani_metrics: None,
    };

    serde_wasm_bindgen::to_value(&report).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn analyze_with_config(config_json: &str, source: &str) -> JsValue {
    let config: SanctifyConfig = serde_json::from_str(config_json).unwrap_or_default();
    let analyzer = Analyzer::new(config);

    let size_warnings = analyzer.analyze_ledger_size(source);
    let unsafe_patterns = analyzer.analyze_unsafe_patterns(source);
    let auth_gaps = analyzer.scan_auth_gaps(source);
    let panic_issues = analyzer.scan_panics(source);
    let arithmetic_issues = analyzer.scan_arithmetic_overflow(source);

    let report = AnalysisReport {
        size_warnings,
        unsafe_patterns,
        auth_gaps,
        panic_issues,
        arithmetic_issues,
        custom_rule_matches: None,
        kani_metrics: None,
    };

    serde_wasm_bindgen::to_value(&report).unwrap_or(JsValue::NULL)
}
