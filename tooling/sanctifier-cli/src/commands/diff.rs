use clap::Args;
use colored::*;
use sanctifier_core::{Analyzer, SanctifyConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::vulndb::VulnDatabase;

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Git reference to compare against (e.g., origin/main, HEAD~1, commit-sha)
    pub git_ref: String,

    /// Path to the contract directory or Cargo.toml
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// Exit with non-zero code if new findings are detected
    #[arg(long)]
    pub fail_on_new: bool,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Path to a custom vulnerability database JSON file
    #[arg(long)]
    pub vuln_db: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct FindingFingerprint {
    code: String,
    location: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct DiffReport {
    added: Vec<FindingSummary>,
    removed: Vec<FindingSummary>,
    persisting: Vec<FindingSummary>,
    summary: DiffSummary,
}

#[derive(Debug, Serialize, Clone)]
struct FindingSummary {
    code: String,
    location: String,
    message: String,
    severity: String,
}

#[derive(Debug, Serialize)]
struct DiffSummary {
    added_count: usize,
    removed_count: usize,
    persisting_count: usize,
    has_new_findings: bool,
}

pub fn exec(args: DiffArgs) -> anyhow::Result<()> {
    let is_json = args.format == "json";

    // Verify we're in a git repository
    if !is_git_repo(&args.path)? {
        let err_msg = "Not a git repository. The diff command requires git.";
        if is_json {
            let err = serde_json::json!({
                "error": err_msg,
                "success": false,
            });
            println!("{}", serde_json::to_string_pretty(&err)?);
        } else {
            eprintln!("{} {}", "❌".red(), err_msg);
        }
        std::process::exit(1);
    }

    // Verify the git ref exists
    if !git_ref_exists(&args.path, &args.git_ref)? {
        let err_msg = format!("Git reference '{}' not found", args.git_ref);
        if is_json {
            let err = serde_json::json!({
                "error": err_msg,
                "success": false,
            });
            println!("{}", serde_json::to_string_pretty(&err)?);
        } else {
            eprintln!("{} {}", "❌".red(), err_msg);
        }
        std::process::exit(1);
    }

    if !is_json {
        println!(
            "{} Analyzing working tree and comparing to {}...",
            "🔍".blue(),
            args.git_ref.bold()
        );
    }

    // Analyze current working tree
    let current_findings = analyze_tree(&args.path, &args.vuln_db, is_json)?;

    // Create temporary directory for the ref checkout
    let temp_dir = tempfile::tempdir()?;
    let ref_path = temp_dir.path();

    // Checkout the ref to temp directory
    checkout_ref_to_temp(&args.path, &args.git_ref, ref_path)?;

    // Analyze the ref
    let ref_findings = analyze_tree(ref_path, &args.vuln_db, is_json)?;

    // Compute diff
    let current_set: HashSet<FindingFingerprint> = current_findings.into_iter().collect();
    let ref_set: HashSet<FindingFingerprint> = ref_findings.into_iter().collect();

    let added: Vec<FindingSummary> = current_set
        .difference(&ref_set)
        .map(|f| FindingSummary {
            code: f.code.clone(),
            location: f.location.clone(),
            message: f.message.clone(),
            severity: infer_severity(&f.code),
        })
        .collect();

    let removed: Vec<FindingSummary> = ref_set
        .difference(&current_set)
        .map(|f| FindingSummary {
            code: f.code.clone(),
            location: f.location.clone(),
            message: f.message.clone(),
            severity: infer_severity(&f.code),
        })
        .collect();

    let persisting: Vec<FindingSummary> = current_set
        .intersection(&ref_set)
        .map(|f| FindingSummary {
            code: f.code.clone(),
            location: f.location.clone(),
            message: f.message.clone(),
            severity: infer_severity(&f.code),
        })
        .collect();

    let report = DiffReport {
        added: added.clone(),
        removed: removed.clone(),
        persisting: persisting.clone(),
        summary: DiffSummary {
            added_count: added.len(),
            removed_count: removed.len(),
            persisting_count: persisting.len(),
            has_new_findings: !added.is_empty(),
        },
    };

    // Output report
    if is_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_text_report(&report, &args.git_ref);
    }

    // Exit with appropriate code
    if args.fail_on_new && !added.is_empty() {
        std::process::exit(1);
    }

    Ok(())
}

fn analyze_tree(
    path: &Path,
    vuln_db_path: &Option<PathBuf>,
    _is_json: bool,
) -> anyhow::Result<Vec<FindingFingerprint>> {
    let config = load_config(path);
    let analyzer = Analyzer::new(config);

    let vuln_db = match vuln_db_path {
        Some(db_path) => VulnDatabase::load(db_path)?,
        None => VulnDatabase::load_default(),
    };

    let mut findings = Vec::new();

    if path.is_dir() {
        collect_findings_from_dir(path, &analyzer, &vuln_db, &mut findings)?;
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        collect_findings_from_file(path, &analyzer, &vuln_db, &mut findings)?;
    }

    Ok(findings)
}

fn collect_findings_from_dir(
    dir: &Path,
    analyzer: &Analyzer,
    vuln_db: &VulnDatabase,
    findings: &mut Vec<FindingFingerprint>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let is_ignored = analyzer
                .config
                .ignore_paths
                .iter()
                .any(|p| path.ends_with(p));
            if is_ignored {
                continue;
            }
            collect_findings_from_dir(&path, analyzer, vuln_db, findings)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            collect_findings_from_file(&path, analyzer, vuln_db, findings)?;
        }
    }
    Ok(())
}

fn collect_findings_from_file(
    file: &Path,
    analyzer: &Analyzer,
    vuln_db: &VulnDatabase,
    findings: &mut Vec<FindingFingerprint>,
) -> anyhow::Result<()> {
    if let Ok(content) = fs::read_to_string(file) {
        let file_name = file.display().to_string();

        // Storage collisions
        for issue in analyzer.scan_storage_collisions(&content) {
            findings.push(FindingFingerprint {
                code: "STORAGE_COLLISION".to_string(),
                location: format!("{}:{}", file_name, issue.location),
                message: format!("{}: {}", issue.key_value, issue.message),
            });
        }

        // Auth gaps
        for gap in analyzer.scan_auth_gaps(&content) {
            findings.push(FindingFingerprint {
                code: "AUTH_GAP".to_string(),
                location: format!("{}:{}", file_name, gap),
                message: format!("Missing authentication in function: {}", gap),
            });
        }

        // Panic issues
        for issue in analyzer.scan_panics(&content) {
            findings.push(FindingFingerprint {
                code: "PANIC_USAGE".to_string(),
                location: format!("{}:{}", file_name, issue.location),
                message: format!("{} in {}", issue.issue_type, issue.function_name),
            });
        }

        // Arithmetic issues
        for issue in analyzer.scan_arithmetic_overflow(&content) {
            findings.push(FindingFingerprint {
                code: "ARITHMETIC_OVERFLOW".to_string(),
                location: format!("{}:{}", file_name, issue.location),
                message: format!(
                    "{} in {}: {}",
                    issue.operation, issue.function_name, issue.suggestion
                ),
            });
        }

        // Size warnings
        for warning in analyzer.analyze_ledger_size(&content) {
            findings.push(FindingFingerprint {
                code: "LEDGER_SIZE_RISK".to_string(),
                location: file_name.clone(),
                message: format!(
                    "{}: {} bytes",
                    warning.struct_name, warning.estimated_size
                ),
            });
        }

        // Unsafe patterns
        for pattern in analyzer.analyze_unsafe_patterns(&content) {
            findings.push(FindingFingerprint {
                code: "UNSAFE_PATTERN".to_string(),
                location: format!("{}:{}", file_name, pattern.line),
                message: format!("{:?}: {}", pattern.pattern_type, pattern.snippet),
            });
        }

        // Event issues
        for issue in analyzer.scan_events(&content) {
            findings.push(FindingFingerprint {
                code: "EVENT_INCONSISTENCY".to_string(),
                location: format!("{}:{}", file_name, issue.location),
                message: format!("{}: {}", issue.event_name, issue.message),
            });
        }

        // Unhandled results
        for issue in analyzer.scan_unhandled_results(&content) {
            findings.push(FindingFingerprint {
                code: "UNHANDLED_RESULT".to_string(),
                location: format!("{}:{}", file_name, issue.location),
                message: format!(
                    "{}: {}",
                    issue.function_name, issue.call_expression
                ),
            });
        }

        // Upgrade patterns
        let upgrade_report = analyzer.analyze_upgrade_patterns(&content);
        for finding in upgrade_report.findings {
            findings.push(FindingFingerprint {
                code: "UPGRADE_RISK".to_string(),
                location: format!("{}:{}", file_name, finding.location),
                message: finding.message.clone(),
            });
        }

        // Vulnerability database matches
        for m in vuln_db.scan(&content, &file_name) {
            findings.push(FindingFingerprint {
                code: m.vuln_id.clone(),
                location: format!("{}:{}", m.file, m.line),
                message: m.description.clone(),
            });
        }

        // Custom rules
        for m in analyzer.analyze_custom_rules(&content, &analyzer.config.custom_rules) {
            findings.push(FindingFingerprint {
                code: "CUSTOM_RULE_MATCH".to_string(),
                location: format!("{}:{}", file_name, m.line),
                message: format!("{}: {}", m.rule_name, m.snippet),
            });
        }
    }

    Ok(())
}

fn is_git_repo(path: &Path) -> anyhow::Result<bool> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(path)
        .output()?;
    Ok(output.status.success())
}

fn git_ref_exists(path: &Path, git_ref: &str) -> anyhow::Result<bool> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("--verify")
        .arg(git_ref)
        .current_dir(path)
        .output()?;
    Ok(output.status.success())
}

fn checkout_ref_to_temp(repo_path: &Path, git_ref: &str, temp_path: &Path) -> anyhow::Result<()> {
    // Get the absolute path to the git repository
    let repo_path = fs::canonicalize(repo_path)?;

    // Use git worktree to checkout the ref
    let output = Command::new("git")
        .arg("worktree")
        .arg("add")
        .arg("--detach")
        .arg(temp_path)
        .arg(git_ref)
        .current_dir(&repo_path)
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to checkout ref to temp directory: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}

fn load_config(path: &Path) -> SanctifyConfig {
    let mut current = if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    };

    loop {
        let config_path = current.join(".sanctify.toml");
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        if !current.pop() {
            break;
        }
    }
    SanctifyConfig::default()
}

fn infer_severity(code: &str) -> String {
    match code {
        "AUTH_GAP" => "critical",
        "ARITHMETIC_OVERFLOW" | "PANIC_USAGE" | "UNHANDLED_RESULT" | "UPGRADE_RISK" => "high",
        "STORAGE_COLLISION" | "LEDGER_SIZE_RISK" | "EVENT_INCONSISTENCY" => "medium",
        _ => "low",
    }
    .to_string()
}

fn print_text_report(report: &DiffReport, git_ref: &str) {
    println!("\n{}", "━".repeat(80).bright_blue());
    println!(
        "{}  Diff Report: Working Tree vs {}",
        "📊".bold(),
        git_ref.bold().cyan()
    );
    println!("{}\n", "━".repeat(80).bright_blue());

    // Summary
    println!("{} Summary:", "📈".bold());
    println!("  {} Added:      {}", "➕".green(), report.summary.added_count);
    println!(
        "  {} Removed:    {}",
        "➖".blue(),
        report.summary.removed_count
    );
    println!(
        "  {} Persisting: {}",
        "🔄".yellow(),
        report.summary.persisting_count
    );
    println!();

    // Added findings (regressions)
    if !report.added.is_empty() {
        println!("{} {} New Findings (Regressions):", "🚨".red(), report.added.len());
        for (idx, finding) in report.added.iter().enumerate() {
            let severity_icon = match finding.severity.as_str() {
                "critical" => "❌".red(),
                "high" => "🔴".red(),
                "medium" => "⚠️".yellow(),
                _ => "ℹ️".blue(),
            };
            println!(
                "  {}. {} [{}] {}",
                idx + 1,
                severity_icon,
                finding.code.bold(),
                finding.severity.to_uppercase()
            );
            println!("     Location: {}", finding.location.dimmed());
            println!("     {}", finding.message);
            println!();
        }
    } else {
        println!("{} No new findings detected! ✨", "✅".green());
        println!();
    }

    // Removed findings (fixes)
    if !report.removed.is_empty() {
        println!("{} {} Fixed Findings:", "🎉".green(), report.removed.len());
        for (idx, finding) in report.removed.iter().enumerate() {
            println!(
                "  {}. [{}] {}",
                idx + 1,
                finding.code.dimmed(),
                finding.location.dimmed()
            );
        }
        println!();
    }

    // Persisting findings
    if !report.persisting.is_empty() && report.persisting.len() <= 10 {
        println!(
            "{} {} Persisting Findings:",
            "🔄".yellow(),
            report.persisting.len()
        );
        for (idx, finding) in report.persisting.iter().enumerate() {
            println!(
                "  {}. [{}] {}",
                idx + 1,
                finding.code.dimmed(),
                finding.location.dimmed()
            );
        }
        println!();
    } else if !report.persisting.is_empty() {
        println!(
            "{} {} Persisting Findings (use JSON format for full list)",
            "🔄".yellow(),
            report.persisting.len()
        );
        println!();
    }

    println!("{}", "━".repeat(80).bright_blue());
    if report.summary.has_new_findings {
        println!(
            "{} Review required: New findings detected",
            "⚠️".yellow().bold()
        );
    } else {
        println!("{} No regressions detected!", "✅".green().bold());
    }
    println!("{}", "━".repeat(80).bright_blue());
}

impl Drop for DiffArgs {
    fn drop(&mut self) {
        // Cleanup any git worktrees we created
        // This is best effort - we don't want to panic in Drop
        let _ = Command::new("git")
            .arg("worktree")
            .arg("prune")
            .current_dir(&self.path)
            .output();
    }
}
