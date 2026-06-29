//! Unified output reporter supporting multiple formats:
//! - table (human-readable TTY)
//! - json (machine-readable)
//! - sarif (SARIF 2.1.0)
//! - junit (JUnit XML for CI viewers)
//!
//! Also handles severity-gated exit codes and CI annotations.

use sanctifier_core::finding_codes;
use sanctifier_core::{
    ArithmeticIssue, CustomRuleMatch, EventIssue, PanicIssue, Severity, SizeWarning,
    SizeWarningLevel, StorageCollisionIssue, UnhandledResultIssue, UnsafePattern, UpgradeReport,
};
use serde::Serialize;
// `SmtInvariantIssue` is behind #[cfg(feature = "smt")] in sanctifier-core
// We'll handle it generically via serde_json::Value
use std::io::{self, Write};
use tracing::{debug, info, warn};

// ── Unified finding model ───────────────────────────────────────────────────────

/// A single finding with severity attached
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub severity: FindingSeverity,
    pub category: String,
    pub message: String,
    pub location: String,
    pub suggestion: Option<String>,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Normalized severity for exit-code purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum FindingSeverity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl FindingSeverity {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "critical" | "error" => FindingSeverity::Critical,
            "high" => FindingSeverity::High,
            "medium" | "warning" => FindingSeverity::Medium,
            "low" => FindingSeverity::Low,
            _ => FindingSeverity::Info,
        }
    }

    pub fn to_severity_gate_index(&self) -> u8 {
        match self {
            FindingSeverity::Critical => 0,
            FindingSeverity::High => 1,
            FindingSeverity::Medium => 2,
            FindingSeverity::Low => 3,
            FindingSeverity::Info => 4,
        }
    }
}

// ── Analysis result ────────────────────────────────────────────────────────────

/// Complete scan result sent to the reporter
#[derive(Debug, Clone, Serialize, Default)]
pub struct AnalysisResult {
    pub project_path: String,
    pub timestamp: String,
    pub vuln_db_version: String,
    pub all_findings: Vec<Finding>,
    pub summary: FindingSummary,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FindingSummary {
    pub total: usize,
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub info: usize,
}

// ── Reporter trait ─────────────────────────────────────────────────────────────

/// Unified reporter that writes findings in a specific format
pub trait Reporter: Send {
    fn write_report(&self, result: &AnalysisResult, writer: &mut dyn Write) -> io::Result<()>;
    fn format_name(&self) -> &str;
}

// ── Reporter factory ───────────────────────────────────────────────────────────

pub fn get_reporter(format: &str) -> Box<dyn Reporter> {
    match format.to_lowercase().as_str() {
        "json" => Box::new(JsonReporter),
        "sarif" => Box::new(SarifReporter),
        "junit" => Box::new(JunitReporter),
        _ => Box::new(TableReporter),
    }
}

// ── Table reporter (human-readable TTY) ────────────────────────────────────────

pub struct TableReporter;

impl Reporter for TableReporter {
    fn format_name(&self) -> &str {
        "table"
    }

    fn write_report(&self, result: &AnalysisResult, writer: &mut dyn Write) -> io::Result<()> {
        let use_color = true; // caller controls via config, but table is always for TTY
        writeln!(writer)?;
        writeln!(
            writer,
            "{} Sanctifier Scan Results",
            if use_color { "\x1b[1m\x1b[36m" } else { "" }
        )?;
        if use_color {
            writeln!(writer, "\x1b[0m")?;
        }
        writeln!(writer, "  Project: {}", result.project_path)?;
        writeln!(writer, "  Timestamp: {}", result.timestamp)?;
        writeln!(writer, "  Vuln DB: v{}", result.vuln_db_version)?;
        writeln!(writer)?;

        // Group by severity
        let mut critical = Vec::new();
        let mut high = Vec::new();
        let mut medium = Vec::new();
        let mut low = Vec::new();
        let mut info = Vec::new();

        for f in &result.all_findings {
            match f.severity {
                FindingSeverity::Critical => critical.push(f),
                FindingSeverity::High => high.push(f),
                FindingSeverity::Medium => medium.push(f),
                FindingSeverity::Low => low.push(f),
                FindingSeverity::Info => info.push(f),
            }
        }

        let mut print_group = |findings: &[&Finding], label: &str, color_code: &str| -> io::Result<()> {
            if findings.is_empty() {
                return Ok(());
            }
            let color = if use_color { color_code } else { "" };
            let reset = if use_color { "\x1b[0m" } else { "" };
            writeln!(writer, "{}{}{}", color, label, reset)?;
            writeln!(writer, "{}", "─".repeat(60))?;
            for f in findings {
                let sev_icon = match f.severity {
                    FindingSeverity::Critical => if use_color { "\x1b[31m" } else { "" },
                    FindingSeverity::High => if use_color { "\x1b[91m" } else { "" },
                    FindingSeverity::Medium => if use_color { "\x1b[33m" } else { "" },
                    FindingSeverity::Low => if use_color { "\x1b[34m" } else { "" },
                    FindingSeverity::Info => if use_color { "\x1b[36m" } else { "" },
                };
                let sev_marker = match f.severity {
                    FindingSeverity::Critical => "❌",
                    FindingSeverity::High => "🔴",
                    FindingSeverity::Medium => "⚠️",
                    FindingSeverity::Low => "ℹ️",
                    FindingSeverity::Info => "💡",
                };

                let file_line = match (&f.file, f.line) {
                    (Some(file), Some(line)) => format!("{}:{}", file, line),
                    (Some(file), None) => file.clone(),
                    _ => f.location.clone(),
                };

                writeln!(
                    writer,
                    "  {} {}[{}]{} {}",
                    sev_marker,
                    sev_icon,
                    f.code,
                    reset,
                    f.message
                )?;
                writeln!(writer, "     File: {}", file_line)?;
                if let Some(ref sug) = f.suggestion {
                    writeln!(writer, "     Suggestion: {}", sug)?;
                }
                writeln!(writer)?;
            }
            Ok(())
        };

        print_group(&critical, "🔴 CRITICAL FINDINGS", "\x1b[31m")?;
        print_group(&high, "🔴 HIGH FINDINGS", "\x1b[91m")?;
        print_group(&medium, "🟡 MEDIUM FINDINGS", "\x1b[33m")?;
        print_group(&low, "🔵 LOW FINDINGS", "\x1b[34m")?;
        print_group(&info, "🟢 INFO", "\x1b[36m")?;

        // Summary
        writeln!(writer, "{}", "─".repeat(60))?;
        writeln!(writer, "Summary:")?;
        writeln!(
            writer,
            "  Critical: {}  High: {}  Medium: {}  Low: {}  Info: {}  Total: {}",
            result.summary.critical,
            result.summary.high,
            result.summary.medium,
            result.summary.low,
            result.summary.info,
            result.summary.total
        )?;
        writeln!(writer)?;

        Ok(())
    }
}

// ── JSON reporter ──────────────────────────────────────────────────────────────

pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn format_name(&self) -> &str {
        "json"
    }

    fn write_report(&self, result: &AnalysisResult, writer: &mut dyn Write) -> io::Result<()> {
        let output = serde_json::to_string_pretty(result)?;
        writeln!(writer, "{}", output)?;
        Ok(())
    }
}

// ── SARIF 2.1.0 reporter ───────────────────────────────────────────────────────

pub struct SarifReporter;

impl Reporter for SarifReporter {
    fn format_name(&self) -> &str {
        "sarif"
    }

    fn write_report(&self, result: &AnalysisResult, writer: &mut dyn Write) -> io::Result<()> {
        let findings = &result.all_findings;
        let finding_codes_list = finding_codes::all_finding_codes();

        // Build SARIF JSON manually for maximum compatibility
        let mut results = Vec::new();
        for (i, finding) in findings.iter().enumerate() {
            let rule_id = finding.code;
            let level = match finding.severity {
                FindingSeverity::Critical | FindingSeverity::High => "error",
                FindingSeverity::Medium => "warning",
                FindingSeverity::Low | FindingSeverity::Info => "note",
            };

            let mut locations = Vec::new();
            if let Some(ref file) = finding.file {
                let uri = format!("file:///{}", file.replace('\\', "/"));
                let physical_location = if let Some(line) = finding.line {
                    serde_json::json!({
                        "artifactLocation": { "uri": uri },
                        "region": {
                            "startLine": line,
                            "startColumn": finding.column.unwrap_or(1)
                        }
                    })
                } else {
                    serde_json::json!({
                        "artifactLocation": { "uri": uri }
                    })
                };
                locations.push(serde_json::json!({
                    "physicalLocation": physical_location
                }));
            }

            let message = serde_json::json!({
                "text": finding.message
            });

            results.push(serde_json::json!({
                "ruleId": rule_id,
                "ruleIndex": i,
                "level": level,
                "message": message,
                "locations": locations,
                "properties": {
                    "category": finding.category,
                    "suggestion": finding.suggestion,
                    "code": finding.code,
                }
            }));
        }

        // Build rules array from finding codes
        let rules: Vec<serde_json::Value> = finding_codes_list
            .iter()
            .enumerate()
            .map(|(i, fc)| {
                serde_json::json!({
                    "id": fc.code,
                    "index": i,
                    "shortDescription": {
                        "text": fc.description
                    },
                    "fullDescription": {
                        "text": fc.description
                    },
                    "properties": {
                        "category": fc.category,
                    }
                })
            })
            .collect();

        let sarif = serde_json::json!({
            "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Documentation/GloballyUniqueIdentifiers/STAR3D/schema/sarif-schema-2.1.0.json",
            "version": "2.1.0",
            "runs": [{
                "tool": {
                    "driver": {
                        "name": "Sanctifier",
                        "version": env!("CARGO_PKG_VERSION"),
                        "informationUri": "https://github.com/Codex723/sanctifier",
                        "rules": rules,
                    }
                },
                "results": results,
                "invocations": [{
                    "executionSuccessful": true,
                    "startTimeUtc": result.timestamp,
                }],
                "properties": {
                    "projectPath": result.project_path,
                    "vulnDbVersion": result.vuln_db_version,
                    "summary": {
                        "total": result.summary.total,
                        "critical": result.summary.critical,
                        "high": result.summary.high,
                        "medium": result.summary.medium,
                        "low": result.summary.low,
                        "info": result.summary.info,
                    }
                }
            }]
        });

        writeln!(writer, "{}", serde_json::to_string_pretty(&sarif)?)?;
        Ok(())
    }
}

// ── JUnit reporter ─────────────────────────────────────────────────────────────

pub struct JunitReporter;

impl Reporter for JunitReporter {
    fn format_name(&self) -> &str {
        "junit"
    }

    fn write_report(&self, result: &AnalysisResult, writer: &mut dyn Write) -> io::Result<()> {
        use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
        use quick_xml::Writer as XmlWriter;

        let write_res = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut xml_writer = XmlWriter::new_with_indent(writer, b' ', 2);

            xml_writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;

            let mut ts = BytesStart::new("testsuite");
            ts.push_attribute(("name", "Sanctifier"));
            ts.push_attribute(("tests", result.summary.total.to_string().as_str()));
            ts.push_attribute(("failures", (result.summary.critical + result.summary.high).to_string().as_str()));
            ts.push_attribute(("errors", "0"));
            ts.push_attribute(("time", "0"));
            xml_writer.write_event(Event::Start(ts))?;

            xml_writer.write_event(Event::Start(BytesStart::new("properties")))?;

            let props = [
                ("project", result.project_path.as_str()),
                ("timestamp", result.timestamp.as_str()),
                ("vuln_db_version", result.vuln_db_version.as_str()),
                ("critical", &result.summary.critical.to_string()),
                ("high", &result.summary.high.to_string()),
                ("medium", &result.summary.medium.to_string()),
                ("low", &result.summary.low.to_string()),
                ("info", &result.summary.info.to_string()),
            ];

            for (name, value) in &props {
                let mut prop = BytesStart::new("property");
                prop.push_attribute(("name", *name));
                prop.push_attribute(("value", *value));
                xml_writer.write_event(Event::Empty(prop))?;
            }

            xml_writer.write_event(Event::End(BytesEnd::new("properties")))?;

            for finding in &result.all_findings {
                let classname = format!("sanctifier.{}", finding.category);
                let name = format!("{} - {}", finding.code, finding.message.chars().take(80).collect::<String>());

                let mut tc = BytesStart::new("testcase");
                tc.push_attribute(("name", name.as_str()));
                tc.push_attribute(("classname", classname.as_str()));

                if finding.severity == FindingSeverity::Critical || finding.severity == FindingSeverity::High {
                    xml_writer.write_event(Event::Start(tc))?;

                    let file_line = match (&finding.file, finding.line) {
                        (Some(f), Some(l)) => format!("{}:{}", f, l),
                        _ => finding.location.clone(),
                    };

                    let mut failure = BytesStart::new("failure");
                    let sev_str = format!("{:?}", finding.severity);
                    failure.push_attribute(("message", finding.message.as_str()));
                    failure.push_attribute(("type", sev_str.as_str()));

                    let text = format!(
                        "Code: {}\nSeverity: {:?}\nLocation: {}\nFile: {}\nSuggestion: {}",
                        finding.code,
                        finding.severity,
                        finding.location,
                        file_line,
                        finding.suggestion.as_deref().unwrap_or("none"),
                    );

                    xml_writer.write_event(Event::Start(failure))?;
                    xml_writer.write_event(Event::Text(BytesText::new(&text)))?;
                    xml_writer.write_event(Event::End(BytesEnd::new("failure")))?;
                    xml_writer.write_event(Event::End(BytesEnd::new("testcase")))?;
                } else {
                    xml_writer.write_event(Event::Start(tc))?;

                    let system_out = format!(
                        "[{}] {} at {} | suggestion: {}",
                        finding.code,
                        finding.message,
                        finding.location,
                        finding.suggestion.as_deref().unwrap_or("none"),
                    );
                    xml_writer.write_event(Event::Start(BytesStart::new("system-out")))?;
                    xml_writer.write_event(Event::Text(BytesText::new(&system_out)))?;
                    xml_writer.write_event(Event::End(BytesEnd::new("system-out")))?;
                    xml_writer.write_event(Event::End(BytesEnd::new("testcase")))?;
                }
            }

            xml_writer.write_event(Event::End(BytesEnd::new("testsuite")))?;
            Ok(())
        })();

        match write_res {
            Ok(()) => Ok(()),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string()))
        }
    }
}

// ── Exit code computation ──────────────────────────────────────────────────────

/// Compute the exit code based on `--fail-on` severity gating
/// Returns 0 for clean (no findings above gate), 1 for findings, 2 for error
pub fn compute_exit_code(findings: &[Finding], fail_on: &str) -> i32 {
    let gate = FindingSeverity::from_str(fail_on).to_severity_gate_index();

    for f in findings {
        if f.severity.to_severity_gate_index() <= gate {
            return 1; // findings exist at or above the gate
        }
    }
    0 // clean
}

/// Documennted exit-code table (used by `--help`)
pub fn exit_code_documentation() -> &'static str {
    "Exit codes:
  0 - No findings (or all findings are below the --fail-on threshold)
  1 - Findings found at or above the --fail-on severity
  2 - Internal error (invalid config, parse failure, etc.)"
}

// ── CI annotation writer ──────────────────────────────────────────────────────

/// Write CI annotations based on findings
pub fn write_ci_annotations(ci_platform: &str, findings: &[Finding]) {
    match ci_platform.to_lowercase().as_str() {
        "github" => write_github_annotations(findings),
        "gitlab" => write_gitlab_annotations(findings),
        _ => debug!("No CI platform configured for annotations"),
    }
}

fn write_github_annotations(findings: &[Finding]) {
    for f in findings {
        if f.severity == FindingSeverity::Info {
            continue; // skip info-level for annotations
        }
        let level = match f.severity {
            FindingSeverity::Critical | FindingSeverity::High => "error",
            FindingSeverity::Medium => "warning",
            FindingSeverity::Low => "notice",
            _ => "notice",
        };
        let file = f.file.as_deref().unwrap_or("unknown");
        let line = f.line.unwrap_or(1);
        let col = f.column.unwrap_or(1);
        let title = format!("[{}] {}", f.code, f.severity.to_severity_gate_index());
        let msg = &f.message;

        // GitHub workflow command: ::error file=...,line=...,col=...,title=...::message
        eprintln!(
            "::{} file={},line={},col={},title={}::{}",
            level, file, line, col, title, msg
        );
    }
}

fn write_gitlab_annotations(findings: &[Finding]) {
    for f in findings {
        if f.severity == FindingSeverity::Info {
            continue;
        }
        let level = match f.severity {
            FindingSeverity::Critical | FindingSeverity::High => "error",
            FindingSeverity::Medium => "warning",
            FindingSeverity::Low => "info",
            _ => "info",
        };
        let file = f.file.as_deref().unwrap_or("unknown");
        let line = f.line.unwrap_or(1);
        // GitLab uses a different format - gl-code-quality-report.json format
        // For now, emit a simplified annotation
        eprintln!(
            "Sanctifier {}: [{}] {} ({}:{})",
            level, f.code, f.message, file, line
        );
    }
}

// ── Error-category ↔ Finding mapping helpers ───────────────────────────────────

/// Convert sanctifier-core analysis types into a flat list of Findings
#[allow(clippy::too_many_arguments)]
pub fn collect_findings(
    auth_gaps: &[String],
    panic_issues: &[PanicIssue],
    arithmetic_issues: &[ArithmeticIssue],
    size_warnings: &[SizeWarning],
    storage_collisions: &[StorageCollisionIssue],
    unsafe_patterns: &[UnsafePattern],
    custom_matches: &[CustomRuleMatch],
    event_issues: &[EventIssue],
    unhandled_results: &[UnhandledResultIssue],
    upgrade_reports: &[UpgradeReport],
    smt_issues: &[serde_json::Value],
    vuln_matches: &[crate::vulndb::VulnMatch],
) -> Vec<Finding> {
    let mut findings = Vec::new();

    for gap in auth_gaps {
        findings.push(Finding {
            code: finding_codes::AUTH_GAP,
            severity: FindingSeverity::Critical,
            category: "authentication".to_string(),
            message: format!("Missing authentication guard in function '{}'", gap),
            location: gap.clone(),
            suggestion: Some(
                "Add require_auth() or require_auth_for_args() to this function".to_string(),
            ),
            file: None,
            line: None,
            column: None,
        });
    }

    for p in panic_issues {
        let sev = if p.issue_type == "panic!" {
            FindingSeverity::High
        } else {
            FindingSeverity::Medium
        };
        findings.push(Finding {
            code: finding_codes::PANIC_USAGE,
            severity: sev,
            category: "panic_handling".to_string(),
            message: format!(
                "{} usage in function '{}' may cause runtime abort",
                p.issue_type, p.function_name
            ),
            location: p.location.clone(),
            suggestion: Some("Replace with Result/Err propagation using `?` operator".to_string()),
            file: None,
            line: None,
            column: None,
        });
    }

    for a in arithmetic_issues {
        findings.push(Finding {
            code: finding_codes::ARITHMETIC_OVERFLOW,
            severity: FindingSeverity::High,
            category: "arithmetic".to_string(),
            message: format!(
                "Unchecked {} operation in '{}'",
                a.operation, a.function_name
            ),
            location: a.location.clone(),
            suggestion: Some(a.suggestion.clone()),
            file: None,
            line: None,
            column: None,
        });
    }

    for w in size_warnings {
        let sev = match w.level {
            SizeWarningLevel::ExceedsLimit => FindingSeverity::High,
            SizeWarningLevel::ApproachingLimit => FindingSeverity::Medium,
        };
        findings.push(Finding {
            code: finding_codes::LEDGER_SIZE_RISK,
            severity: sev,
            category: "storage_limits".to_string(),
            message: format!(
                "'{}' estimated size {} bytes exceeds limit {}",
                w.struct_name, w.estimated_size, w.limit
            ),
            location: w.struct_name.clone(),
            suggestion: Some("Reduce struct size or increase ledger_limit".to_string()),
            file: None,
            line: None,
            column: None,
        });
    }

    for c in storage_collisions {
        findings.push(Finding {
            code: finding_codes::STORAGE_COLLISION,
            severity: FindingSeverity::High,
            category: "storage_keys".to_string(),
            message: c.message.clone(),
            location: c.location.clone(),
            suggestion: Some("Use unique key prefixes to avoid collisions".to_string()),
            file: None,
            line: None,
            column: None,
        });
    }

    for u in unsafe_patterns {
        findings.push(Finding {
            code: finding_codes::UNSAFE_PATTERN,
            severity: FindingSeverity::Medium,
            category: "unsafe_patterns".to_string(),
            message: format!("{:?} pattern detected", u.pattern_type),
            location: format!("line {}", u.line),
            suggestion: None,
            file: None,
            line: Some(u.line),
            column: None,
        });
    }

    for cm in custom_matches {
        let sev = match cm.severity {
            sanctifier_core::RuleSeverity::Error => FindingSeverity::Critical,
            sanctifier_core::RuleSeverity::Warning => FindingSeverity::Medium,
            sanctifier_core::RuleSeverity::Info => FindingSeverity::Info,
        };
        findings.push(Finding {
            code: finding_codes::CUSTOM_RULE_MATCH,
            severity: sev,
            category: "custom_rule".to_string(),
            message: format!("Custom rule '{}' matched", cm.rule_name),
            location: cm.snippet.clone(),
            suggestion: None,
            file: None,
            line: Some(cm.line),
            column: None,
        });
    }

    for e in event_issues {
        let sev = match e.issue_type {
            sanctifier_core::EventIssueType::InconsistentSchema => FindingSeverity::Medium,
            sanctifier_core::EventIssueType::OptimizableTopic => FindingSeverity::Low,
        };
        findings.push(Finding {
            code: finding_codes::EVENT_INCONSISTENCY,
            severity: sev,
            category: "events".to_string(),
            message: e.message.clone(),
            location: e.location.clone(),
            suggestion: Some("Standardize event topic counts; use symbol_short!".to_string()),
            file: None,
            line: None,
            column: None,
        });
    }

    for r in unhandled_results {
        findings.push(Finding {
            code: finding_codes::UNHANDLED_RESULT,
            severity: FindingSeverity::Medium,
            category: "logic".to_string(),
            message: r.message.clone(),
            location: r.location.clone(),
            suggestion: Some("Use ?, match, or .unwrap()/.expect() to handle Result".to_string()),
            file: None,
            line: None,
            column: None,
        });
    }

    for report in upgrade_reports {
        for f in &report.findings {
            findings.push(Finding {
                code: finding_codes::UPGRADE_RISK,
                severity: FindingSeverity::Medium,
                category: "upgrades".to_string(),
                message: f.message.clone(),
                location: f.location.clone(),
                suggestion: Some(f.suggestion.clone()),
                file: None,
                line: None,
                column: None,
            });
        }
    }

    for s in smt_issues {
        let desc = s.get("description").and_then(|v| v.as_str()).unwrap_or("SMT invariant violation");
        let loc = s.get("location").and_then(|v| v.as_str()).unwrap_or("<unknown>");
        let fn_name = s.get("function_name").and_then(|v| v.as_str()).unwrap_or("<unknown>");
        findings.push(Finding {
            code: finding_codes::SMT_INVARIANT_VIOLATION,
            severity: FindingSeverity::Critical,
            category: "formal_verification".to_string(),
            message: desc.to_string(),
            location: loc.to_string(),
            suggestion: Some("Review the invariant and add bounds checking".to_string()),
            file: None,
            line: None,
            column: None,
        });
    }

    for vm in vuln_matches {
        let sev = FindingSeverity::from_str(&vm.severity);
        findings.push(Finding {
            code: Box::leak(vm.vuln_id.clone().into_boxed_str()),
            severity: sev,
            category: vm.category.clone(),
            message: vm.description.clone(),
            location: format!("{}:{}", vm.file, vm.line),
            suggestion: Some(vm.recommendation.clone()),
            file: Some(vm.file.clone()),
            line: Some(vm.line),
            column: None,
        });
    }

    findings
}

/// Compute summary statistics from findings
pub fn compute_summary(findings: &[Finding]) -> FindingSummary {
    let mut summary = FindingSummary::default();
    summary.total = findings.len();
    for f in findings {
        match f.severity {
            FindingSeverity::Critical => summary.critical += 1,
            FindingSeverity::High => summary.high += 1,
            FindingSeverity::Medium => summary.medium += 1,
            FindingSeverity::Low => summary.low += 1,
            FindingSeverity::Info => summary.info += 1,
        }
    }
    summary
}