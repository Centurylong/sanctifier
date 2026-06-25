use sanctifier_core::{
    finding_codes, Analyzer, ArithmeticIssue, CustomRule, CustomRuleMatch, EventIssue, PanicIssue,
    SanctifyConfig, SizeWarning, StorageCollisionIssue, UnhandledResultIssue, UnsafePattern,
    UpgradeReport,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Finding {
    code: &'static str,
    category: &'static str,
    severity: &'static str,
    message: String,
    location: String,
    function_name: Option<String>,
    line: Option<usize>,
}

#[derive(Serialize)]
struct AnalysisReport {
    findings: Vec<Finding>,
    summary: Summary,
    raw: RawReport,
}

#[derive(Serialize, Default)]
struct Summary {
    total: usize,
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
    info: usize,
}

#[derive(Serialize)]
struct RawReport {
    size_warnings: Vec<SizeWarning>,
    unsafe_patterns: Vec<UnsafePattern>,
    auth_gaps: Vec<String>,
    panic_issues: Vec<PanicIssue>,
    arithmetic_issues: Vec<ArithmeticIssue>,
    storage_collisions: Vec<StorageCollisionIssue>,
    unhandled_results: Vec<UnhandledResultIssue>,
    event_issues: Vec<EventIssue>,
    upgrade_report: UpgradeReport,
    custom_rule_matches: Vec<CustomRuleMatch>,
}

fn build_report(
    analyzer: &Analyzer,
    source: &str,
    custom_rules: &[CustomRule],
) -> AnalysisReport {
    let size_warnings = analyzer.analyze_ledger_size(source);
    let unsafe_patterns = analyzer.analyze_unsafe_patterns(source);
    let auth_gaps = analyzer.scan_auth_gaps(source);
    let panic_issues = analyzer.scan_panics(source);
    let arithmetic_issues = analyzer.scan_arithmetic_overflow(source);
    let storage_collisions = analyzer.scan_storage_collisions(source);
    let unhandled_results = analyzer.scan_unhandled_results(source);
    let event_issues = analyzer.scan_events(source);
    let upgrade_report = analyzer.analyze_upgrade_patterns(source);
    let custom_rule_matches = if custom_rules.is_empty() {
        Vec::new()
    } else {
        analyzer.analyze_custom_rules(source, custom_rules)
    };

    let mut findings: Vec<Finding> = Vec::new();

    for w in &size_warnings {
        findings.push(Finding {
            code: finding_codes::LEDGER_SIZE_RISK,
            category: "storage_limits",
            severity: "high",
            message: format!(
                "Storage entry {} estimated {} bytes (limit {})",
                w.struct_name, w.estimated_size, w.limit
            ),
            location: w.struct_name.clone(),
            function_name: None,
            line: None,
        });
    }

    for p in &unsafe_patterns {
        findings.push(Finding {
            code: finding_codes::UNSAFE_PATTERN,
            category: "unsafe_patterns",
            severity: "medium",
            message: format!("Unsafe pattern: {:?}", p.pattern_type),
            location: format!("line {}", p.line),
            function_name: None,
            line: Some(p.line),
        });
    }

    for fname in &auth_gaps {
        findings.push(Finding {
            code: finding_codes::AUTH_GAP,
            category: "authentication",
            severity: "high",
            message: format!("Function `{}` mutates state without require_auth", fname),
            location: fname.clone(),
            function_name: Some(fname.clone()),
            line: None,
        });
    }

    for p in &panic_issues {
        findings.push(Finding {
            code: finding_codes::PANIC_USAGE,
            category: "panic_handling",
            severity: "medium",
            message: format!(
                "Function `{}` uses `{}` which may panic at runtime",
                p.function_name, p.issue_type
            ),
            location: p.location.clone(),
            function_name: Some(p.function_name.clone()),
            line: None,
        });
    }

    for a in &arithmetic_issues {
        findings.push(Finding {
            code: finding_codes::ARITHMETIC_OVERFLOW,
            category: "arithmetic",
            severity: "high",
            message: format!(
                "Function `{}` uses unchecked `{}`. {}",
                a.function_name, a.operation, a.suggestion
            ),
            location: a.location.clone(),
            function_name: Some(a.function_name.clone()),
            line: None,
        });
    }

    for s in &storage_collisions {
        findings.push(Finding {
            code: finding_codes::STORAGE_COLLISION,
            category: "storage_keys",
            severity: "high",
            message: s.message.clone(),
            location: s.location.clone(),
            function_name: None,
            line: None,
        });
    }

    for u in &unhandled_results {
        findings.push(Finding {
            code: finding_codes::UNHANDLED_RESULT,
            category: "logic",
            severity: "low",
            message: u.message.clone(),
            location: u.location.clone(),
            function_name: Some(u.function_name.clone()),
            line: None,
        });
    }

    for e in &event_issues {
        findings.push(Finding {
            code: finding_codes::EVENT_INCONSISTENCY,
            category: "events",
            severity: "info",
            message: e.message.clone(),
            location: e.location.clone(),
            function_name: Some(e.function_name.clone()),
            line: None,
        });
    }

    for u in &upgrade_report.findings {
        findings.push(Finding {
            code: finding_codes::UPGRADE_RISK,
            category: "upgrades",
            severity: "high",
            message: u.message.clone(),
            location: u.location.clone(),
            function_name: u.function_name.clone(),
            line: None,
        });
    }

    for m in &custom_rule_matches {
        let sev = match m.severity {
            sanctifier_core::RuleSeverity::Info => "info",
            sanctifier_core::RuleSeverity::Warning => "medium",
            sanctifier_core::RuleSeverity::Error => "high",
        };
        findings.push(Finding {
            code: finding_codes::CUSTOM_RULE_MATCH,
            category: "custom_rule",
            severity: sev,
            message: format!("Custom rule `{}` matched", m.rule_name),
            location: format!("line {}", m.line),
            function_name: None,
            line: Some(m.line),
        });
    }

    let mut summary = Summary::default();
    summary.total = findings.len();
    for f in &findings {
        match f.severity {
            "critical" => summary.critical += 1,
            "high" => summary.high += 1,
            "medium" => summary.medium += 1,
            "low" => summary.low += 1,
            _ => summary.info += 1,
        }
    }

    AnalysisReport {
        findings,
        summary,
        raw: RawReport {
            size_warnings,
            unsafe_patterns,
            auth_gaps,
            panic_issues,
            arithmetic_issues,
            storage_collisions,
            unhandled_results,
            event_issues,
            upgrade_report,
            custom_rule_matches,
        },
    }
}

#[wasm_bindgen]
pub fn analyze(source: &str) -> JsValue {
    let analyzer = Analyzer::new(SanctifyConfig::default());
    let report = build_report(&analyzer, source, &[]);
    serde_wasm_bindgen::to_value(&report).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn analyze_with_config(config_json: &str, source: &str) -> JsValue {
    let config: SanctifyConfig = serde_json::from_str(config_json).unwrap_or_default();
    let custom_rules = config.custom_rules.clone();
    let analyzer = Analyzer::new(config);
    let report = build_report(&analyzer, source, &custom_rules);
    serde_wasm_bindgen::to_value(&report).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn finding_code_catalog() -> JsValue {
    let codes = finding_codes::all_finding_codes();
    serde_wasm_bindgen::to_value(&codes).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
