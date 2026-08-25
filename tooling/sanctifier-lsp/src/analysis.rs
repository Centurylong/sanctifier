//! Turning sanctifier-core findings into LSP diagnostics.
//!
//! Kept apart from the protocol and dispatch layers so the mapping — which is
//! where the editor-visible behaviour actually lives — can be tested without
//! standing up a server or a JSON-RPC stream.

use sanctifier_core::{Analyzer, SanctifyConfig};
use serde::Serialize;

/// LSP `DiagnosticSeverity`. The protocol has four levels; the engine has
/// five, so `low` and `info` both land on Information — deliberately, because
/// Hint is rendered as a barely-visible dotted underline in most editors and
/// would effectively hide findings the engine did choose to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(into = "u8")]
pub enum Severity {
    Error = 1,
    Warning = 2,
    Information = 3,
}

impl From<Severity> for u8 {
    fn from(s: Severity) -> u8 {
        s as u8
    }
}

pub fn severity_for(engine_severity: &str) -> Severity {
    match engine_severity {
        "critical" | "high" => Severity::Error,
        "medium" => Severity::Warning,
        _ => Severity::Information,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Severity,
    pub code: String,
    pub source: &'static str,
    pub message: String,
}

/// A finding as the analyzer reports it, flattened to what a diagnostic needs.
pub struct Finding {
    pub code: String,
    pub message: String,
    /// 1-based, as the engine reports. `None` when it could not attribute one.
    pub line: Option<usize>,
    pub severity: String,
}

/// Build the range to underline for a finding.
///
/// LSP lines are 0-based while the engine reports 1-based, and an
/// unattributed finding must still be reportable — dropping it would mean the
/// editor silently shows fewer problems than `sanctifier analyze` does. Those
/// are pinned to the first line instead.
pub fn range_for(line: Option<usize>, source: &str) -> Range {
    let zero_based = line.and_then(|l| l.checked_sub(1)).unwrap_or(0) as u32;

    // Clamp to the document: a finding attributed past the end of the file
    // (stale analysis, or a detector counting expanded macro lines) would make
    // some clients drop the whole diagnostic batch.
    let last_line = source.lines().count().saturating_sub(1) as u32;
    let line_index = zero_based.min(last_line);

    let line_len = source
        .lines()
        .nth(line_index as usize)
        .map(|l| l.chars().count())
        .unwrap_or(0) as u32;

    Range {
        start: Position {
            line: line_index,
            character: 0,
        },
        end: Position {
            line: line_index,
            character: line_len,
        },
    }
}

pub fn to_diagnostic(finding: &Finding, source: &str) -> Diagnostic {
    Diagnostic {
        range: range_for(finding.line, source),
        severity: severity_for(&finding.severity),
        code: finding.code.clone(),
        source: "sanctifier",
        message: finding.message.clone(),
    }
}

/// Pull the line number out of a core `location` string.
///
/// Several detectors report position as a `"function_name:line"` context
/// string rather than a numeric field. The CLI prints that verbatim, but an
/// editor needs the number to place the squiggle, so it is parsed here rather
/// than every finding collapsing onto line 1.
pub fn line_from_location(location: &str) -> Option<usize> {
    location.rsplit_once(':')?.1.trim().parse().ok()
}

/// Run the analyzer over one document and collect its findings.
///
/// Every detector call is a pure function of the source text, so this is
/// deliberately stateless: re-analysing on save is cheap and cannot serve a
/// stale result, which is what an incremental cache would risk for the sake of
/// milliseconds the editor does not notice.
pub fn analyze(source: &str) -> Vec<Finding> {
    let analyzer = Analyzer::new(SanctifyConfig::default());
    let mut findings = Vec::new();

    for gap in analyzer.scan_auth_gaps(source) {
        findings.push(Finding {
            code: "S001".to_string(),
            message: gap,
            line: None,
            severity: "high".to_string(),
        });
    }
    for issue in analyzer.scan_panics(source) {
        findings.push(Finding {
            code: "S002".to_string(),
            message: format!(
                "Function `{}` uses `{}` which may panic at runtime",
                issue.function_name, issue.issue_type
            ),
            line: line_from_location(&issue.location),
            severity: "medium".to_string(),
        });
    }
    for issue in analyzer.scan_arithmetic_overflow(source) {
        findings.push(Finding {
            code: "S003".to_string(),
            message: format!(
                "Function `{}` uses unchecked `{}`. {}",
                issue.function_name, issue.operation, issue.suggestion
            ),
            line: line_from_location(&issue.location),
            severity: "high".to_string(),
        });
    }
    for pattern in analyzer.analyze_unsafe_patterns(source) {
        findings.push(Finding {
            code: "S006".to_string(),
            message: format!("Unsafe pattern: {:?}", pattern.pattern_type),
            line: Some(pattern.line),
            severity: "medium".to_string(),
        });
    }

    findings
}

/// Documentation shown when the editor hovers a line carrying a finding.
pub fn hover_markdown(findings: &[Finding], line_zero_based: u32) -> Option<String> {
    let target = line_zero_based as usize + 1;
    let on_line: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.line.unwrap_or(1) == target)
        .collect();

    if on_line.is_empty() {
        return None;
    }

    let mut out = String::new();
    for (i, finding) in on_line.iter().enumerate() {
        if i > 0 {
            out.push_str("\n\n---\n\n");
        }
        out.push_str(&format!(
            "**{} · {}**\n\n{}",
            finding.code,
            finding.severity.to_uppercase(),
            finding.message
        ));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VULNERABLE: &str = r#"use soroban_sdk::{contract, contractimpl, Address, Env};

#[contract]
pub struct Vault;

#[contractimpl]
impl Vault {
    pub fn withdraw(env: Env, from: Address, amount: i128) {
        let balance: i128 = env.storage().persistent().get(&from).unwrap();
        env.storage().persistent().set(&from, &(balance - amount));
    }
}
"#;

    #[test]
    fn engine_severities_map_onto_the_protocol() {
        assert_eq!(severity_for("critical"), Severity::Error);
        assert_eq!(severity_for("high"), Severity::Error);
        assert_eq!(severity_for("medium"), Severity::Warning);
        assert_eq!(severity_for("low"), Severity::Information);
        assert_eq!(severity_for("info"), Severity::Information);
        // An unknown severity must still surface, not vanish.
        assert_eq!(severity_for("something-new"), Severity::Information);
    }

    #[test]
    fn severity_serializes_as_the_protocol_number() {
        let json = serde_json::to_string(&Severity::Error).unwrap();
        assert_eq!(json, "1");
    }

    #[test]
    fn line_numbers_are_converted_from_one_based_to_zero_based() {
        let range = range_for(Some(3), "a\nbb\nccc\ndddd\n");
        assert_eq!(range.start.line, 2);
        assert_eq!(range.end.character, 3, "should span the whole line");
    }

    #[test]
    fn unattributed_findings_are_pinned_to_the_first_line() {
        // Dropping them would make the editor show fewer problems than the CLI.
        let range = range_for(None, "a\nbb\n");
        assert_eq!(range.start.line, 0);
    }

    #[test]
    fn out_of_range_lines_are_clamped_to_the_document() {
        // Some clients discard an entire diagnostic batch if one range points
        // past the end of the file.
        let range = range_for(Some(9999), "a\nbb\n");
        assert_eq!(range.start.line, 1);
    }

    #[test]
    fn line_zero_does_not_underflow() {
        let range = range_for(Some(0), "a\nbb\n");
        assert_eq!(range.start.line, 0);
    }

    #[test]
    fn line_numbers_are_recovered_from_location_strings() {
        assert_eq!(line_from_location("withdraw:12"), Some(12));
        assert_eq!(line_from_location("line 9"), None);
        assert_eq!(line_from_location("no_colon"), None);
        assert_eq!(line_from_location("mod::path::fn:41"), Some(41));
    }

    #[test]
    fn a_vulnerable_contract_produces_diagnostics() {
        let findings = analyze(VULNERABLE);
        assert!(
            !findings.is_empty(),
            "expected findings for a deliberately vulnerable contract"
        );

        for finding in &findings {
            let diagnostic = to_diagnostic(finding, VULNERABLE);
            assert_eq!(diagnostic.source, "sanctifier");
            assert!(!diagnostic.code.is_empty());
            assert!(!diagnostic.message.is_empty());
        }
    }

    #[test]
    fn analysis_never_panics_on_arbitrary_text() {
        // A panic here takes the whole language server down and the editor
        // stops reporting anything at all until it is restarted.
        for source in ["", "not rust {{{", "fn main( {", "\u{feff}"] {
            let _ = analyze(source);
        }
    }

    #[test]
    fn hover_returns_documentation_on_a_line_carrying_a_finding() {
        let findings = analyze(VULNERABLE);
        let with_line = findings
            .iter()
            .find_map(|f| f.line)
            .expect("expected at least one attributed finding");

        let markdown = hover_markdown(&findings, with_line as u32 - 1)
            .expect("a line with a finding should produce hover content");
        assert!(markdown.contains("**"), "hover should be markdown");
    }

    #[test]
    fn hover_returns_nothing_on_a_line_with_no_findings() {
        // Line 1 is excluded: unattributed findings are pinned there, matching
        // `range_for`, so it is the one line that can report without a
        // detector having named it.
        let findings = analyze(VULNERABLE);
        let attributed: Vec<usize> = findings.iter().filter_map(|f| f.line).collect();
        let clean_line = (2..=VULNERABLE.lines().count())
            .find(|l| !attributed.contains(l))
            .expect("fixture should have at least one finding-free line past the first");

        assert!(hover_markdown(&findings, clean_line as u32 - 1).is_none());
    }
}
