//! Suppression baseline: snapshot and filter pre-existing findings.
//!
//! # Workflow
//!
//! ```text
//! sanctifier baseline          # snapshot current findings → .sanctify-baseline.json
//! sanctifier analyze           # report only NEW findings (baseline entries suppressed)
//! sanctifier baseline --update # refresh the baseline after intentional changes
//! ```
//!
//! # File format
//!
//! `.sanctify-baseline.json` is a JSON object:
//! ```json
//! {
//!   "version": 1,
//!   "created_at": "2025-06-21T22:00:00Z",
//!   "total_suppressed": 3,
//!   "entries": [
//!     {
//!       "fingerprint": "a1b2c3…",
//!       "code": "S001",
//!       "path": "src/contract.rs",
//!       "context": "transfer"
//!     }
//!   ]
//! }
//! ```
//!
//! # Fingerprint stability
//!
//! Each entry is identified by `SHA-256(code | ":" | normalized_path | ":" | context)`.
//! `normalized_path` strips line/column numbers and converts backslashes to forward
//! slashes so the fingerprint is resilient to line shifts and OS differences.
//! `context` is the most stable semantic identifier available for each finding type
//! (function name, key value, pattern type, …) — never the line number.

use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

pub const BASELINE_FILE: &str = ".sanctify-baseline.json";

// ── Public types ──────────────────────────────────────────────────────────────

/// Normalised, flat representation of a single finding, suitable for hashing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct FlatFinding {
    /// Sanctifier finding code, e.g. `"S001"`.
    pub code: String,
    /// File path with line numbers stripped and separators normalised.
    pub path: String,
    /// Stable semantic identifier (function name, key, etc.) — never a line number.
    pub context: String,
}

impl FlatFinding {
    pub fn new(code: impl Into<String>, raw_location: &str, context: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            path: normalize_path(raw_location),
            context: context.into(),
        }
    }

    /// Compute the stable SHA-256 fingerprint for this finding.
    pub fn fingerprint(&self) -> String {
        fingerprint_parts(&self.code, &self.path, &self.context)
    }
}

/// A single entry stored in `.sanctify-baseline.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    /// Hex-encoded SHA-256 of `code:path:context`.
    pub fingerprint: String,
    pub code: String,
    pub path: String,
    pub context: String,
}

impl BaselineEntry {
    pub fn from_flat(f: &FlatFinding) -> Self {
        Self {
            fingerprint: f.fingerprint(),
            code: f.code.clone(),
            path: f.path.clone(),
            context: f.context.clone(),
        }
    }
}

/// The root object of `.sanctify-baseline.json`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Baseline {
    pub version: u32,
    pub created_at: String,
    pub total_suppressed: usize,
    pub entries: Vec<BaselineEntry>,
}

/// One finding in a baseline trend report.
///
/// The stable fingerprint remains the identity, while the other fields keep
/// the report useful to humans without re-running analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrendFinding {
    pub fingerprint: String,
    pub code: String,
    pub path: String,
    pub context: String,
}

impl TrendFinding {
    fn from_flat(finding: &FlatFinding) -> Self {
        Self {
            fingerprint: finding.fingerprint(),
            code: finding.code.clone(),
            path: finding.path.clone(),
            context: finding.context.clone(),
        }
    }

    fn from_entry(entry: &BaselineEntry) -> Self {
        Self {
            fingerprint: entry.fingerprint.clone(),
            code: entry.code.clone(),
            path: entry.path.clone(),
            context: entry.context.clone(),
        }
    }
}

/// Deterministic comparison of current findings against a stored baseline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BaselineTrend {
    pub added: Vec<TrendFinding>,
    pub fixed: Vec<TrendFinding>,
    pub persistent: Vec<TrendFinding>,
}

impl BaselineTrend {
    /// Compare the current scan to the stored baseline using the same stable
    /// fingerprints that baseline suppression already relies on.
    pub fn compare(baseline: &Baseline, current: &[FlatFinding]) -> Self {
        let baseline_by_fp: BTreeMap<String, TrendFinding> = baseline
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.fingerprint.clone(),
                    TrendFinding::from_entry(entry),
                )
            })
            .collect();
        let current_by_fp: BTreeMap<String, TrendFinding> = current
            .iter()
            .map(|finding| {
                let trend = TrendFinding::from_flat(finding);
                (trend.fingerprint.clone(), trend)
            })
            .collect();

        let mut added: Vec<TrendFinding> = current_by_fp
            .iter()
            .filter(|(fingerprint, _)| !baseline_by_fp.contains_key(*fingerprint))
            .map(|(_, finding)| finding.clone())
            .collect();
        let mut fixed: Vec<TrendFinding> = baseline_by_fp
            .iter()
            .filter(|(fingerprint, _)| !current_by_fp.contains_key(*fingerprint))
            .map(|(_, finding)| finding.clone())
            .collect();
        let mut persistent: Vec<TrendFinding> = current_by_fp
            .iter()
            .filter(|(fingerprint, _)| baseline_by_fp.contains_key(*fingerprint))
            .map(|(_, finding)| finding.clone())
            .collect();

        sort_trend_findings(&mut added);
        sort_trend_findings(&mut fixed);
        sort_trend_findings(&mut persistent);

        Self {
            added,
            fixed,
            persistent,
        }
    }

    /// Render the trend as deterministic Markdown.
    pub fn render_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Sanctifier Baseline Trend\n\n");
        out.push_str("| Status | Count |\n| --- | ---: |\n");
        out.push_str(&format!("| Added | {} |\n", self.added.len()));
        out.push_str(&format!("| Fixed | {} |\n", self.fixed.len()));
        out.push_str(&format!("| Persistent | {} |\n\n", self.persistent.len()));

        render_markdown_findings(&mut out, "Added", &self.added);
        render_markdown_findings(&mut out, "Fixed", &self.fixed);
        render_markdown_findings(&mut out, "Persistent", &self.persistent);
        out
    }

    /// Render the trend as deterministic, standalone HTML.
    pub fn render_html(&self) -> String {
        let mut out = String::new();
        out.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
        out.push_str("<meta charset=\"utf-8\">\n<title>Sanctifier Baseline Trend</title>\n");
        out.push_str("</head>\n<body>\n<main>\n<h1>Sanctifier Baseline Trend</h1>\n");
        out.push_str("<table><thead><tr><th>Status</th><th>Count</th></tr></thead><tbody>\n");
        out.push_str(&format!("<tr><td>Added</td><td>{}</td></tr>\n", self.added.len()));
        out.push_str(&format!("<tr><td>Fixed</td><td>{}</td></tr>\n", self.fixed.len()));
        out.push_str(&format!(
            "<tr><td>Persistent</td><td>{}</td></tr>\n",
            self.persistent.len()
        ));
        out.push_str("</tbody></table>\n");

        render_html_findings(&mut out, "Added", &self.added);
        render_html_findings(&mut out, "Fixed", &self.fixed);
        render_html_findings(&mut out, "Persistent", &self.persistent);
        out.push_str("</main>\n</body>\n</html>\n");
        out
    }
}

fn sort_trend_findings(findings: &mut [TrendFinding]) {
    findings.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.context.cmp(&b.context))
            .then_with(|| a.fingerprint.cmp(&b.fingerprint))
    });
}

fn render_markdown_findings(out: &mut String, heading: &str, findings: &[TrendFinding]) {
    out.push_str(&format!("## {heading}\n\n"));
    if findings.is_empty() {
        out.push_str("_None._\n\n");
        return;
    }

    out.push_str("| Code | Path | Context |\n| --- | --- | --- |\n");
    for finding in findings {
        out.push_str(&format!(
            "| {} | {} | {} |\n",
            escape_markdown_cell(&finding.code),
            escape_markdown_cell(&finding.path),
            escape_markdown_cell(&finding.context),
        ));
    }
    out.push('\n');
}

fn render_html_findings(out: &mut String, heading: &str, findings: &[TrendFinding]) {
    out.push_str(&format!("<section><h2>{}</h2>\n", escape_html(heading)));
    if findings.is_empty() {
        out.push_str("<p>None.</p></section>\n");
        return;
    }

    out.push_str("<table><thead><tr><th>Code</th><th>Path</th><th>Context</th></tr></thead><tbody>\n");
    for finding in findings {
        out.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape_html(&finding.code),
            escape_html(&finding.path),
            escape_html(&finding.context),
        ));
    }
    out.push_str("</tbody></table></section>\n");
}

fn escape_markdown_cell(value: &str) -> String {
    // Markdown tables may be rendered with inline HTML enabled. Treat finding
    // path/context as untrusted text, not as executable HTML tags/entities.
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\n', "<br>")
        .replace('\r', "")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Compute a stable hex-encoded SHA-256 fingerprint.
pub fn fingerprint_parts(code: &str, path: &str, context: &str) -> String {
    let mut h = Sha256::new();
    h.update(code.as_bytes());
    h.update(b":");
    h.update(path.as_bytes());
    h.update(b":");
    h.update(context.as_bytes());
    hex::encode(h.finalize())
}

/// Strip trailing `:N` or `:N:M` patterns and normalise path separators.
///
/// `"src\\contract.rs:42:3"` → `"src/contract.rs"`
pub fn normalize_path(raw: &str) -> String {
    // Use lazy_static-style init via once_cell or just compile each time (fine for a CLI).
    let line_col = Regex::new(r":\d+(?::\d+)?$").expect("static regex");
    let stripped = line_col.replace(raw, "").to_string();
    stripped.replace('\\', "/")
}

/// Load the baseline from `dir/.sanctify-baseline.json`.
/// Returns `None` if the file does not exist.
pub fn load_baseline(dir: &Path) -> anyhow::Result<Option<Baseline>> {
    let path = dir.join(BASELINE_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&path)?;
    let baseline: Baseline = serde_json::from_str(&text)?;
    Ok(Some(baseline))
}

/// Write the baseline to `dir/.sanctify-baseline.json`.
pub fn save_baseline(dir: &Path, entries: Vec<BaselineEntry>) -> anyhow::Result<()> {
    let baseline = Baseline {
        version: 1,
        created_at: iso8601_now(),
        total_suppressed: entries.len(),
        entries,
    };
    let json = serde_json::to_string_pretty(&baseline)?;
    fs::write(dir.join(BASELINE_FILE), json)?;
    Ok(())
}

/// Given a baseline and the current set of findings, return:
/// - The findings whose fingerprints are **not** in the baseline (new findings)
/// - The baseline entries that no longer appear in the current scan (stale entries)
pub fn apply_baseline<'a>(
    baseline: &'a Baseline,
    current: &[FlatFinding],
) -> (Vec<FlatFinding>, Vec<&'a BaselineEntry>) {
    let base_fps: HashSet<&str> = baseline
        .entries
        .iter()
        .map(|e| e.fingerprint.as_str())
        .collect();
    let current_fps: HashSet<String> = current.iter().map(|f| f.fingerprint()).collect();

    let new_findings: Vec<FlatFinding> = current
        .iter()
        .filter(|f| !base_fps.contains(f.fingerprint().as_str()))
        .cloned()
        .collect();

    let stale: Vec<&'a BaselineEntry> = baseline
        .entries
        .iter()
        .filter(|e| !current_fps.contains(&e.fingerprint))
        .collect();

    (new_findings, stale)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn iso8601_now() -> String {
    // Avoid pulling in chrono — use Unix seconds from std.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Format as a rough ISO-8601 UTC string (good enough for metadata).
    let (y, mo, d, h, mi, s) = seconds_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn seconds_to_ymd_hms(mut s: u64) -> (u64, u64, u64, u64, u64, u64) {
    let sec = s % 60;
    s /= 60;
    let min = s % 60;
    s /= 60;
    let hour = s % 24;
    s /= 24;
    // Days since Unix epoch (2000-03-01 reference for Gregorian calculation)
    let days = s;
    let (y, mo, d) = civil_from_days(days);
    (y, mo, d, hour, min, sec)
}

fn civil_from_days(z: u64) -> (u64, u64, u64) {
    let z = z as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m as u64, d as u64)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_is_stable_across_line_changes() {
        // Same finding code + path + context → same fingerprint regardless of line numbers.
        let f1 = FlatFinding::new("S001", "src/contract.rs:10", "transfer");
        let f2 = FlatFinding::new("S001", "src/contract.rs:42", "transfer");
        assert_eq!(f1.fingerprint(), f2.fingerprint());
    }

    #[test]
    fn fingerprint_differs_for_different_codes() {
        let f1 = FlatFinding::new("S001", "src/contract.rs:10", "transfer");
        let f2 = FlatFinding::new("S002", "src/contract.rs:10", "transfer");
        assert_ne!(f1.fingerprint(), f2.fingerprint());
    }

    #[test]
    fn fingerprint_differs_for_different_context() {
        let f1 = FlatFinding::new("S001", "src/contract.rs", "transfer");
        let f2 = FlatFinding::new("S001", "src/contract.rs", "withdraw");
        assert_ne!(f1.fingerprint(), f2.fingerprint());
    }

    #[test]
    fn normalize_path_strips_line_numbers() {
        assert_eq!(normalize_path("src/foo.rs:42"), "src/foo.rs");
        assert_eq!(normalize_path("src/foo.rs:42:3"), "src/foo.rs");
        assert_eq!(normalize_path("src/foo.rs"), "src/foo.rs");
    }

    #[test]
    fn normalize_path_converts_backslashes() {
        assert_eq!(normalize_path("src\\foo.rs:5"), "src/foo.rs");
    }

    #[test]
    fn apply_baseline_filters_known_findings() {
        let finding = FlatFinding::new("S001", "src/contract.rs", "transfer");
        let entry = BaselineEntry::from_flat(&finding);
        let baseline = Baseline {
            version: 1,
            created_at: String::new(),
            total_suppressed: 1,
            entries: vec![entry],
        };

        let (new, stale) = apply_baseline(&baseline, std::slice::from_ref(&finding));
        assert!(new.is_empty(), "baselined finding should be suppressed");
        assert!(stale.is_empty(), "no stale entries expected");
    }

    #[test]
    fn apply_baseline_surfaces_new_findings() {
        let old = FlatFinding::new("S001", "src/contract.rs", "transfer");
        let entry = BaselineEntry::from_flat(&old);
        let baseline = Baseline {
            version: 1,
            created_at: String::new(),
            total_suppressed: 1,
            entries: vec![entry],
        };

        let new_finding = FlatFinding::new("S002", "src/contract.rs", "withdraw");
        let (new, stale) = apply_baseline(&baseline, &[old, new_finding.clone()]);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].code, "S002");
        assert!(stale.is_empty());
    }

    #[test]
    fn apply_baseline_detects_stale_entries() {
        let old = FlatFinding::new("S001", "src/deleted.rs", "transfer");
        let entry = BaselineEntry::from_flat(&old);
        let baseline = Baseline {
            version: 1,
            created_at: String::new(),
            total_suppressed: 1,
            entries: vec![entry],
        };

        // Current scan finds nothing — old baseline entry is now stale.
        let (new, stale) = apply_baseline(&baseline, &[]);
        assert!(new.is_empty());
        assert_eq!(stale.len(), 1);
    }

    #[test]
    fn trend_categorizes_added_fixed_and_persistent_findings() {
        let fixed = FlatFinding::new("S001", "src/fixed.rs:10", "old");
        let persistent = FlatFinding::new("S002", "src/same.rs:20", "same");
        let added = FlatFinding::new("S003", "src/new.rs:30", "new");
        let baseline = Baseline {
            version: 1,
            created_at: String::new(),
            total_suppressed: 2,
            entries: vec![
                BaselineEntry::from_flat(&persistent),
                BaselineEntry::from_flat(&fixed),
            ],
        };

        let trend = BaselineTrend::compare(&baseline, &[added.clone(), persistent.clone()]);

        assert_eq!(trend.added.len(), 1);
        assert_eq!(trend.added[0].fingerprint, added.fingerprint());
        assert_eq!(trend.fixed.len(), 1);
        assert_eq!(trend.fixed[0].fingerprint, fixed.fingerprint());
        assert_eq!(trend.persistent.len(), 1);
        assert_eq!(trend.persistent[0].fingerprint, persistent.fingerprint());
    }

    #[test]
    fn trend_renderers_are_deterministic_and_escape_content() {
        let unsafe_finding = FlatFinding::new("S099", "src/<demo>.rs:1", "left|right & <tag>");
        let baseline = Baseline {
            version: 1,
            created_at: String::new(),
            total_suppressed: 0,
            entries: vec![],
        };
        let trend = BaselineTrend::compare(&baseline, &[unsafe_finding]);

        let markdown = trend.render_markdown();
        assert!(markdown.contains("| Added | 1 |"));
        assert!(markdown.contains("left\\|right &amp; &lt;tag&gt;"));
        assert!(markdown.contains("src/&lt;demo&gt;.rs:1"));

        let html = trend.render_html();
        assert!(html.contains("<tr><td>Added</td><td>1</td></tr>"));
        assert!(html.contains("src/&lt;demo&gt;.rs"));
        assert!(html.contains("left|right &amp; &lt;tag&gt;"));
    }
}
