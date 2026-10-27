//! Inline suppression audit model and parser (`// sanctifier-ignore:`).
//!
//! Scans Rust source files for inline suppression directives, extracts the
//! suppressed rule code, optional author tag, and justification, and flags
//! missing or weak justifications so suppressions stay accountable during
//! security review.

use serde::{Deserialize, Serialize};

/// Minimum character length required for a justification to be considered strong.
pub const MIN_JUSTIFICATION_CHARS: usize = 10;

/// Minimum word count required for a justification to be considered strong.
pub const MIN_JUSTIFICATION_WORDS: usize = 2;

const WEAK_PLACEHOLDERS: &[&str] = &[
    "todo",
    "tbd",
    "fixme",
    "later",
    "ignore",
    "ignored",
    "ok",
    "safe",
    "fine",
    "none",
    "n/a",
    "na",
    "temp",
    "temporary",
    "workaround",
    "hack",
    "skip",
];

/// Classification of an inline suppression's justification quality.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionStatus {
    /// Suppression includes both a rule code and a substantive justification.
    Valid,
    /// Suppression is missing a justification after `-`.
    MissingJustification,
    /// Suppression has a justification that is too brief or a generic placeholder.
    WeakJustification,
    /// Directive is missing a target rule code.
    MissingRuleCode,
}

/// Audit record for a single `// sanctifier-ignore` directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressionAuditEntry {
    pub file: String,
    pub line: usize,
    pub rule_code: String,
    pub author: Option<String>,
    pub justification: Option<String>,
    pub status: SuppressionStatus,
    pub flag_reason: Option<String>,
}

/// Aggregated audit report across all scanned files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressionAuditReport {
    pub total_suppressions: usize,
    pub valid_count: usize,
    pub missing_justification_count: usize,
    pub weak_justification_count: usize,
    pub missing_rule_code_count: usize,
    pub entries: Vec<SuppressionAuditEntry>,
}

impl SuppressionAuditReport {
    /// Returns `true` if any suppression is missing a rule code or has a missing/weak justification.
    pub fn has_flagged_entries(&self) -> bool {
        self.missing_justification_count > 0
            || self.weak_justification_count > 0
            || self.missing_rule_code_count > 0
    }
}

/// Parses an optional leading author tag such as `(@alice)` or `[alice]` from a justification.
fn extract_author_and_justification(raw: &str) -> (Option<String>, String) {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix('(') {
        if let Some((author, remainder)) = rest.split_once(')') {
            let author_clean = author.trim().trim_start_matches('@').trim();
            if !author_clean.is_empty() && !author_clean.contains(' ') {
                return (Some(author_clean.to_string()), remainder.trim().to_string());
            }
        }
    }
    if let Some(rest) = trimmed.strip_prefix('[') {
        if let Some((author, remainder)) = rest.split_once(']') {
            let author_clean = author
                .trim()
                .strip_prefix("author:")
                .unwrap_or(author)
                .trim()
                .trim_start_matches('@')
                .trim();
            if !author_clean.is_empty() && !author_clean.contains(' ') {
                return (Some(author_clean.to_string()), remainder.trim().to_string());
            }
        }
    }
    (None, trimmed.to_string())
}

/// Evaluates whether a non-empty justification string is substantive or weak.
pub fn evaluate_justification(justification: &str) -> (SuppressionStatus, Option<String>) {
    let trimmed = justification.trim();
    if trimmed.is_empty() {
        return (
            SuppressionStatus::MissingJustification,
            Some("Inline suppression is missing a justification after '-'".to_string()),
        );
    }

    let normalized = trimmed
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_ascii_lowercase();

    if WEAK_PLACEHOLDERS.contains(&normalized.as_str()) {
        return (
            SuppressionStatus::WeakJustification,
            Some(format!(
        "Justification '{trimmed}' is a placeholder; explain why this finding is safe to suppress"
      )),
        );
    }

    let word_count = trimmed.split_whitespace().count();
    if trimmed.chars().count() < MIN_JUSTIFICATION_CHARS || word_count < MIN_JUSTIFICATION_WORDS {
        return (
      SuppressionStatus::WeakJustification,
      Some(format!(
        "Justification '{trimmed}' is too brief (requires at least {MIN_JUSTIFICATION_WORDS} words and {MIN_JUSTIFICATION_CHARS} characters)"
      )),
    );
    }

    (SuppressionStatus::Valid, None)
}

/// Scans a single Rust source file for `// sanctifier-ignore` directives and returns audit entries.
pub fn audit_source_suppressions(file_path: &str, source: &str) -> Vec<SuppressionAuditEntry> {
    let mut entries = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let line_num = idx + 1;
        let Some(marker_pos) = line.find("// sanctifier-ignore") else {
            continue;
        };

        let after_prefix = &line[marker_pos + "// sanctifier-ignore".len()..];
        let rest = after_prefix
            .strip_prefix(':')
            .unwrap_or(after_prefix)
            .trim();

        if rest.is_empty() {
            entries.push(SuppressionAuditEntry {
                file: file_path.to_string(),
                line: line_num,
                rule_code: String::new(),
                author: None,
                justification: None,
                status: SuppressionStatus::MissingRuleCode,
                flag_reason: Some(
                    "Inline suppression is missing both a rule code and a justification"
                        .to_string(),
                ),
            });
            continue;
        }

        if let Some((raw_code, raw_justification)) = rest.split_once('-') {
            let code = raw_code.trim();
            let (author, justification_text) = extract_author_and_justification(raw_justification);

            if code.is_empty() {
                entries.push(SuppressionAuditEntry {
                    file: file_path.to_string(),
                    line: line_num,
                    rule_code: String::new(),
                    author,
                    justification: if justification_text.is_empty() {
                        None
                    } else {
                        Some(justification_text)
                    },
                    status: SuppressionStatus::MissingRuleCode,
                    flag_reason: Some(
                        "Inline suppression is missing a rule code before '-'".to_string(),
                    ),
                });
                continue;
            }

            let (status, flag_reason) = evaluate_justification(&justification_text);
            entries.push(SuppressionAuditEntry {
                file: file_path.to_string(),
                line: line_num,
                rule_code: code.to_string(),
                author,
                justification: if justification_text.is_empty() {
                    None
                } else {
                    Some(justification_text)
                },
                status,
                flag_reason,
            });
        } else {
            let code = rest.trim();
            entries.push(SuppressionAuditEntry {
                file: file_path.to_string(),
                line: line_num,
                rule_code: code.to_string(),
                author: None,
                justification: None,
                status: SuppressionStatus::MissingJustification,
                flag_reason: Some(
                    "Inline suppression is missing a '- <justification>' explanation".to_string(),
                ),
            });
        }
    }

    entries
}

/// Builds an aggregated `SuppressionAuditReport` from individual audit entries.
pub fn build_suppression_audit_report(
    entries: Vec<SuppressionAuditEntry>,
) -> SuppressionAuditReport {
    let mut report = SuppressionAuditReport {
        total_suppressions: entries.len(),
        ..SuppressionAuditReport::default()
    };

    for entry in &entries {
        match entry.status {
            SuppressionStatus::Valid => report.valid_count += 1,
            SuppressionStatus::MissingJustification => report.missing_justification_count += 1,
            SuppressionStatus::WeakJustification => report.weak_justification_count += 1,
            SuppressionStatus::MissingRuleCode => report.missing_rule_code_count += 1,
        }
    }

    report.entries = entries;
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audits_valid_missing_and_weak_suppressions() {
        let src = r#"
pub fn transfer(env: Env) {
    // sanctifier-ignore: S001 -(@alice) Admin check enforced by upstream router contract
    let a = 1;
    // sanctifier-ignore: S002 - todo
    let b = 2;
    // sanctifier-ignore: S003
    let c = 3;
    // sanctifier-ignore: - missing rule code
    let d = 4;
}
"#;

        let entries = audit_source_suppressions("contracts/token/src/lib.rs", src);
        let report = build_suppression_audit_report(entries);

        assert_eq!(report.total_suppressions, 4);
        assert_eq!(report.valid_count, 1);
        assert_eq!(report.weak_justification_count, 1);
        assert_eq!(report.missing_justification_count, 1);
        assert_eq!(report.missing_rule_code_count, 1);
        assert!(report.has_flagged_entries());

        assert_eq!(report.entries[0].rule_code, "S001");
        assert_eq!(report.entries[0].author.as_deref(), Some("alice"));
        assert_eq!(report.entries[0].status, SuppressionStatus::Valid);

        assert_eq!(report.entries[1].rule_code, "S002");
        assert_eq!(
            report.entries[1].status,
            SuppressionStatus::WeakJustification
        );

        assert_eq!(report.entries[2].rule_code, "S003");
        assert_eq!(
            report.entries[2].status,
            SuppressionStatus::MissingJustification
        );

        assert_eq!(report.entries[3].status, SuppressionStatus::MissingRuleCode);
    }
}
