use anyhow::Context;
use clap::Args;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args, Debug)]
pub struct BadgeArgs {
    /// Path to Sanctifier JSON report (from `sanctifier analyze --format json`)
    #[arg(short, long, default_value = "sanctifier-report.json")]
    pub report: PathBuf,

    /// Where to write generated badge SVG
    #[arg(long, default_value = "sanctifier-security.svg")]
    pub svg_output: PathBuf,

    /// Where to write generated markdown snippet
    #[arg(long)]
    pub markdown_output: Option<PathBuf>,

    /// Public URL for the SVG (used by markdown output). Falls back to local SVG path.
    #[arg(long)]
    pub badge_url: Option<String>,

    /// Badge content to render: status, severity, grade, or trend.
    #[arg(long, default_value = "status")]
    pub variant: String,
}

#[derive(Debug, Deserialize)]
struct AnalyzeReport {
    summary: AnalyzeSummary,
    #[serde(default)]
    baseline: AnalyzeBaseline,
    #[serde(default)]
    findings: AnalyzeFindings,
    #[serde(default)]
    vulnerability_db_matches: Vec<AnalyzeVulnerabilityMatch>,
}

#[derive(Debug, Deserialize)]
struct AnalyzeSummary {
    total_findings: usize,
    has_critical: bool,
    has_high: bool,
}

#[derive(Debug, Default, Deserialize)]
struct AnalyzeBaseline {
    #[serde(default)]
    stale_entries: Vec<serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct AnalyzeFindings {
    #[serde(default)]
    auth_gaps: Vec<serde_json::Value>,
    #[serde(default)]
    panic_issues: Vec<AnalyzePanicIssue>,
    #[serde(default)]
    arithmetic_issues: Vec<serde_json::Value>,
    #[serde(default)]
    ledger_size_warnings: Vec<AnalyzeSizeWarning>,
    #[serde(default)]
    unhandled_results: Vec<serde_json::Value>,
    #[serde(default)]
    smt_issues: Vec<serde_json::Value>,
    #[serde(default)]
    storage_collisions: Vec<serde_json::Value>,
    #[serde(default)]
    unsafe_patterns: Vec<serde_json::Value>,
    #[serde(default)]
    custom_rules: Vec<AnalyzeCustomRule>,
    #[serde(default)]
    event_issues: Vec<serde_json::Value>,
    #[serde(default)]
    upgrade_risks: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct AnalyzePanicIssue {
    issue_type: String,
}

#[derive(Debug, Deserialize)]
struct AnalyzeSizeWarning {
    level: String,
}

#[derive(Debug, Deserialize)]
struct AnalyzeCustomRule {
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnalyzeVulnerabilityMatch {
    #[serde(default)]
    severity: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SeverityCounts {
    critical: usize,
    high: usize,
    medium: usize,
    low: usize,
}

impl SeverityCounts {
    fn add_named_severity(&mut self, severity: Option<&str>) {
        match severity.unwrap_or("medium").to_ascii_lowercase().as_str() {
            "critical" => self.critical += 1,
            "high" | "error" => self.high += 1,
            "medium" | "warning" => self.medium += 1,
            "low" | "info" => self.low += 1,
            _ => self.medium += 1,
        }
    }

    fn grade(self) -> &'static str {
        if self.critical > 0 {
            "F"
        } else if self.high > 0 {
            "D"
        } else if self.medium > 0 {
            "C"
        } else if self.low > 0 {
            "B"
        } else {
            "A"
        }
    }

    fn badge_color(self) -> &'static str {
        if self.critical > 0 {
            SecurityStatus::Critical.color()
        } else if self.high + self.medium + self.low > 0 {
            SecurityStatus::Warning.color()
        } else {
            SecurityStatus::Secure.color()
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct BadgePresentation {
    label: &'static str,
    value: String,
    color: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecurityStatus {
    Secure,
    Warning,
    Critical,
}

impl SecurityStatus {
    fn text(self) -> &'static str {
        match self {
            SecurityStatus::Secure => "Secure",
            SecurityStatus::Warning => "Warning",
            SecurityStatus::Critical => "Critical",
        }
    }

    fn color(self) -> &'static str {
        match self {
            SecurityStatus::Secure => "#2ea043",
            SecurityStatus::Warning => "#fb8c00",
            SecurityStatus::Critical => "#d73a49",
        }
    }
}

pub fn exec(args: BadgeArgs) -> anyhow::Result<()> {
    let report_content = fs::read_to_string(&args.report)
        .with_context(|| format!("failed to read report file: {}", args.report.display()))?;
    let report: AnalyzeReport = serde_json::from_str(&report_content)
        .with_context(|| format!("failed to parse JSON report: {}", args.report.display()))?;

    let presentation = badge_presentation(&report, &args.variant)?;
    let svg = generate_badge_svg(
        presentation.label,
        &presentation.value,
        presentation.color,
    );

    write_text_file(&args.svg_output, &svg)?;

    let markdown_url = args
        .badge_url
        .unwrap_or_else(|| normalize_path_for_markdown(&args.svg_output));
    let markdown = format!(
        "![{}: {}]({})",
        presentation.label,
        presentation.value,
        markdown_destination(&markdown_url)
    );

    if let Some(md_path) = args.markdown_output {
        write_text_file(&md_path, &(markdown.clone() + "\n"))?;
        println!("Markdown snippet written to {}", md_path.display());
    } else {
        println!("{}", markdown);
    }

    println!("Badge generated at {}", args.svg_output.display());
    println!("Badge value: {}", presentation.value);
    Ok(())
}

fn write_text_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }
    }
    fs::write(path, content)
        .with_context(|| format!("failed to write file: {}", path.display()))?;
    Ok(())
}

fn derive_status(summary: &AnalyzeSummary) -> SecurityStatus {
    if summary.has_critical {
        SecurityStatus::Critical
    } else if summary.has_high || summary.total_findings > 0 {
        SecurityStatus::Warning
    } else {
        SecurityStatus::Secure
    }
}

fn severity_counts(report: &AnalyzeReport) -> SeverityCounts {
    let critical_panics = report
        .findings
        .panic_issues
        .iter()
        .filter(|issue| issue.issue_type == "panic!")
        .count();
    let high_panics = report
        .findings
        .panic_issues
        .len()
        .saturating_sub(critical_panics);

    let critical = report.findings.auth_gaps.len() + critical_panics;
    let high = high_panics
        + report.findings.arithmetic_issues.len()
        + report.findings.unhandled_results.len()
        + report.findings.smt_issues.len()
        + report
            .findings
            .ledger_size_warnings
            .iter()
            .filter(|warning| warning.level == "ExceedsLimit")
            .count();
    let medium = report.findings.storage_collisions.len()
        + report.findings.unsafe_patterns.len()
        + report.findings.event_issues.len()
        + report.findings.upgrade_risks.len();
    let low = report
        .findings
        .ledger_size_warnings
        .iter()
        .filter(|warning| warning.level != "ExceedsLimit")
        .count();

    let mut counts = SeverityCounts {
        critical,
        high,
        medium,
        low,
    };
    for rule in &report.findings.custom_rules {
        counts.add_named_severity(rule.severity.as_deref());
    }
    for finding in &report.vulnerability_db_matches {
        counts.add_named_severity(finding.severity.as_deref());
    }
    counts
}

fn badge_presentation(report: &AnalyzeReport, variant: &str) -> anyhow::Result<BadgePresentation> {
    let status = derive_status(&report.summary);
    match variant {
        "status" => Ok(BadgePresentation {
            label: "Sanctifier",
            value: status.text().to_string(),
            color: status.color(),
        }),
        "severity" => {
            let counts = severity_counts(report);
            Ok(BadgePresentation {
                label: "Sanctifier severity",
                value: format!(
                    "C:{} H:{} M:{} L:{}",
                    counts.critical, counts.high, counts.medium, counts.low
                ),
                color: counts.badge_color(),
            })
        }
        "grade" => {
            let counts = severity_counts(report);
            Ok(BadgePresentation {
                label: "Sanctifier grade",
                value: counts.grade().to_string(),
                color: counts.badge_color(),
            })
        }
        "trend" => Ok(BadgePresentation {
            label: "Sanctifier trend",
            value: format!(
                "+{} / -{}",
                report.summary.total_findings,
                report.baseline.stale_entries.len()
            ),
            color: if report.summary.total_findings == 0 {
                SecurityStatus::Secure.color()
            } else {
                status.color()
            },
        }),
        other => anyhow::bail!(
            "unknown badge variant '{other}'; expected status, severity, grade, or trend"
        ),
    }
}

fn generate_badge_svg(label: &str, status: &str, status_color: &str) -> String {
    let label_width = text_width(label);
    let status_width = text_width(status);
    let total_width = label_width + status_width;
    let status_x = label_width;
    let label_text_x = label_width / 2;
    let status_text_x = label_width + (status_width / 2);

    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{total_width}\" height=\"20\" role=\"img\" aria-label=\"{label}: {status}\">\
<linearGradient id=\"g\" x2=\"0\" y2=\"100%\">\
<stop offset=\"0\" stop-color=\"#fff\" stop-opacity=\".7\"/>\
<stop offset=\".1\" stop-color=\"#aaa\" stop-opacity=\".1\"/>\
<stop offset=\".9\" stop-opacity=\".3\"/>\
<stop offset=\"1\" stop-opacity=\".5\"/>\
</linearGradient>\
<clipPath id=\"r\"><rect width=\"{total_width}\" height=\"20\" rx=\"3\" fill=\"#fff\"/></clipPath>\
<g clip-path=\"url(#r)\">\
<rect width=\"{label_width}\" height=\"20\" fill=\"#555\"/>\
<rect x=\"{status_x}\" width=\"{status_width}\" height=\"20\" fill=\"{status_color}\"/>\
<rect width=\"{total_width}\" height=\"20\" fill=\"url(#g)\"/>\
</g>\
<g fill=\"#fff\" text-anchor=\"middle\" font-family=\"DejaVu Sans,Verdana,Geneva,sans-serif\" font-size=\"11\">\
<text x=\"{label_text_x}\" y=\"15\" fill=\"#010101\" fill-opacity=\".3\">{label}</text>\
<text x=\"{label_text_x}\" y=\"14\">{label}</text>\
<text x=\"{status_text_x}\" y=\"15\" fill=\"#010101\" fill-opacity=\".3\">{status}</text>\
<text x=\"{status_text_x}\" y=\"14\">{status}</text>\
</g>\
</svg>"
    )
}

fn text_width(text: &str) -> usize {
    let padded = (text.chars().count() * 7) + 10;
    padded.max(28)
}

/// Serialize a Markdown image destination without changing the underlying URL.
/// CommonMark angle destinations preserve spaces and parentheses; escaping
/// punctuation prevents a literal URL entity from changing on parse.
fn markdown_destination(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('<');
    for ch in value.chars() {
        match ch {
            '\\' | '<' | '>' | '&' => {
                result.push('\\');
                result.push(ch);
            }
            '\n' => result.push_str("%0A"),
            '\r' => result.push_str("%0D"),
            _ => result.push(ch),
        }
    }
    result.push('>');
    result
}

fn normalize_path_for_markdown(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn derive_status_handles_critical() {
        let summary = AnalyzeSummary {
            total_findings: 12,
            has_critical: true,
            has_high: true,
        };
        assert_eq!(derive_status(&summary), SecurityStatus::Critical);
    }

    #[test]
    fn derive_status_handles_warning() {
        let summary = AnalyzeSummary {
            total_findings: 1,
            has_critical: false,
            has_high: false,
        };
        assert_eq!(derive_status(&summary), SecurityStatus::Warning);
    }

    #[test]
    fn derive_status_handles_secure() {
        let summary = AnalyzeSummary {
            total_findings: 0,
            has_critical: false,
            has_high: false,
        };
        assert_eq!(derive_status(&summary), SecurityStatus::Secure);
    }

    #[test]
    fn generate_svg_contains_expected_text() {
        let svg = generate_badge_svg("Sanctifier", "Secure", "#2ea043");
        assert!(svg.contains("Sanctifier"));
        assert!(svg.contains("Secure"));
        assert!(svg.contains("#2ea043"));
    }

    #[test]
    fn variants_use_current_report_and_baseline_semantics() {
        let report: AnalyzeReport = serde_json::from_str(r#"{
          "summary": {
            "total_findings": 14,
            "has_critical": true,
            "has_high": true
          },
          "baseline": {
            "stale_entries": [{}, {}]
          },
          "findings": {
            "auth_gaps": [{}],
            "panic_issues": [
              {"issue_type": "panic!"},
              {"issue_type": "unwrap"}
            ],
            "arithmetic_issues": [{}],
            "ledger_size_warnings": [
              {"level": "ExceedsLimit"},
              {"level": "ApproachingLimit"}
            ],
            "unhandled_results": [{}],
            "smt_issues": [{}],
            "storage_collisions": [{}, {}],
            "unsafe_patterns": [{}],
            "custom_rules": [{}],
            "event_issues": [{}],
            "upgrade_risks": [{}]
          }
        }"#).expect("report fixture should parse");

        assert_eq!(
            severity_counts(&report),
            SeverityCounts {
                critical: 2,
                high: 5,
                medium: 6,
                low: 1,
            }
        );
        assert_eq!(
            badge_presentation(&report, "severity").unwrap().value,
            "C:2 H:5 M:6 L:1"
        );
        assert_eq!(
            badge_presentation(&report, "grade").unwrap().value,
            "F"
        );
        assert_eq!(
            badge_presentation(&report, "trend").unwrap().value,
            "+14 / -2"
        );
    }

    #[test]
    fn custom_rule_severity_is_preserved() {
        let report: AnalyzeReport = serde_json::from_str(r#"{
          "summary": {
            "total_findings": 3,
            "has_critical": false,
            "has_high": true
          },
          "findings": {
            "custom_rules": [
              {"severity": "error"},
              {"severity": "warning"},
              {"severity": "info"}
            ]
          }
        }"#).expect("report fixture should parse");

        let counts = severity_counts(&report);
        assert_eq!(
            counts,
            SeverityCounts {
                critical: 0,
                high: 1,
                medium: 1,
                low: 1,
            }
        );
        let grade = badge_presentation(&report, "grade").unwrap();
        assert_eq!(grade.value, "D");
        assert_eq!(grade.color, SecurityStatus::Warning.color());
    }

    #[test]
    fn vulnerability_database_matches_contribute_to_severity_and_grade() {
        let report: AnalyzeReport = serde_json::from_str(r#"{
          "summary": {
            "total_findings": 0,
            "has_critical": false,
            "has_high": false
          },
          "vulnerability_db_matches": [
            {"severity": "critical"},
            {"severity": "low"}
          ]
        }"#).expect("report fixture should parse");

        assert_eq!(
            severity_counts(&report),
            SeverityCounts {
                critical: 1,
                high: 0,
                medium: 0,
                low: 1,
            }
        );
        assert_eq!(
            badge_presentation(&report, "grade").unwrap().value,
            "F"
        );
    }

    #[test]
    fn grade_uses_worst_severity() {
        let cases = [
            (SeverityCounts::default(), "A"),
            (
                SeverityCounts {
                    low: 1,
                    ..SeverityCounts::default()
                },
                "B",
            ),
            (
                SeverityCounts {
                    medium: 1,
                    ..SeverityCounts::default()
                },
                "C",
            ),
            (
                SeverityCounts {
                    high: 1,
                    ..SeverityCounts::default()
                },
                "D",
            ),
            (
                SeverityCounts {
                    critical: 1,
                    ..SeverityCounts::default()
                },
                "F",
            ),
        ];

        for (counts, expected) in cases {
            assert_eq!(counts.grade(), expected);
        }
    }

    #[test]
    fn unknown_variant_is_rejected() {
        let report: AnalyzeReport = serde_json::from_str(r#"{
          "summary": {
            "total_findings": 0,
            "has_critical": false,
            "has_high": false
          }
        }"#).unwrap();
        assert!(badge_presentation(&report, "unknown").is_err());
    }

    #[test]
    fn exec_writes_svg_and_markdown_files() {
        let tmp = TempDir::new().expect("temp dir should be created");
        let report_path = tmp.path().join("report.json");
        let svg_path = tmp.path().join("badges").join("status.svg");
        let md_path = tmp.path().join("badges").join("status.md");

        let report = r#"{
  "summary": {
    "total_findings": 0,
    "has_critical": false,
    "has_high": false
  }
}"#;
        fs::write(&report_path, report).expect("report fixture should be written");

        let args = BadgeArgs {
            report: report_path,
            svg_output: svg_path.clone(),
            markdown_output: Some(md_path.clone()),
            badge_url: Some("https://example.com/sanctifier-security.svg".to_string()),
            variant: "status".to_string(),
        };
        exec(args).expect("badge command should succeed");

        let svg = fs::read_to_string(svg_path).expect("svg should exist");
        let md = fs::read_to_string(md_path).expect("markdown should exist");

        assert!(svg.contains("Sanctifier"));
        assert!(svg.contains("Secure"));
        assert!(md.contains("https://example.com/sanctifier-security.svg"));
    }
}
