use crate::commands::webhook::{
    send_scan_completed_webhooks, ScanWebhookPayload, ScanWebhookSummary,
};
use clap::Args;
use colored::*;
use sanctifier_core::{Analyzer, ArithmeticIssue, SizeWarning, UnsafePattern};
use crate::llm;
use tokio::runtime::Runtime;

#[derive(Args, Debug)]
pub struct AnalyzeArgs {
    /// Path to the contract directory or Cargo.toml
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Limit for ledger entry size in bytes
    #[arg(short, long, default_value = "64000")]
    pub limit: usize,

    /// Enable LLM-assisted explanations for findings
    #[arg(long, default_value_t = false)]
    pub llm_explain: bool,
}

pub fn exec(args: AnalyzeArgs) -> anyhow::Result<()> {
    let path = &args.path;
    let format = &args.format;
    let _limit = args.limit;
    let is_json = format == "json";

    if !is_soroban_project(path) {
        if is_json {
            let err = serde_json::json!({
                "error": format!("{:?} is not a valid Soroban project", path),
                "success": false,
            });
            println!("{}", serde_json::to_string_pretty(&err)?);
        } else {
            eprintln!(
                "{} Error: {:?} is not a valid Soroban project. (Missing Cargo.toml with 'soroban-sdk' dependency)",
                "❌".red(),
                path
            );
        }
        std::process::exit(1);
    }

    if is_json {
        eprintln!(
            "{} Sanctifier: Valid Soroban project found at {:?}",
            "✨".green(),
            path
        );
        eprintln!("{} Analyzing contract at {:?}...", "🔍".blue(), path);
    } else {
        println!(
            "{} Sanctifier: Valid Soroban project found at {:?}",
            "✨".green(),
            path
        );
        println!("{} Analyzing contract at {:?}...", "🔍".blue(), path);
        use std::io::{self, Write};
        io::stdout().flush().ok();
    }

    let mut config = load_config(path);
    config.ledger_limit = args.limit; // Apply CLI limit to config
    let analyzer = Analyzer::new(config);

    // Load vulnerability database
    let vuln_db = match &args.vuln_db {
        Some(db_path) => {
            if !is_json {
                println!(
                    "{} Loading custom vulnerability database from {:?}",
                    "📦".blue(),
                    db_path
                );
            }
            VulnDatabase::load(db_path)?
        }
        None => {
            if !is_json {
                println!(
                    "{} Loading built-in vulnerability database (v{})",
                    "📦".blue(),
                    VulnDatabase::load_default().version
                );
            }
            VulnDatabase::load_default()
        }
    };

    let mut collisions = Vec::new();
    let mut size_warnings = Vec::new();
    let mut unsafe_patterns = Vec::new();
    let mut auth_gaps = Vec::new();
    let mut panic_issues = Vec::new();
    let mut arithmetic_issues = Vec::new();
    let mut custom_matches = Vec::new();
    let mut vuln_matches: Vec<VulnMatch> = Vec::new();
    let mut event_issues = Vec::new();
    let mut unhandled_results = Vec::new();
    let mut upgrade_reports = Vec::new();
    let mut smt_issues = Vec::new();

    if path.is_dir() {
        walk_dir(
            path,
            &analyzer,
            &vuln_db,
            &mut collisions,
            &mut size_warnings,
            &mut unsafe_patterns,
            &mut auth_gaps,
            &mut panic_issues,
            &mut arithmetic_issues,
            &mut custom_matches,
            &mut vuln_matches,
            &mut event_issues,
            &mut unhandled_results,
            &mut upgrade_reports,
            &mut smt_issues,
        )?;
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        if let Ok(content) = fs::read_to_string(path) {
            let file_name = path.display().to_string();
            collisions.extend(analyzer.scan_storage_collisions(&content));
            size_warnings.extend(analyzer.analyze_ledger_size(&content));
            unsafe_patterns.extend(analyzer.analyze_unsafe_patterns(&content));
            auth_gaps.extend(analyzer.scan_auth_gaps(&content));
            panic_issues.extend(analyzer.scan_panics(&content));
            arithmetic_issues.extend(analyzer.scan_arithmetic_overflow(&content));
            custom_matches
                .extend(analyzer.analyze_custom_rules(&content, &analyzer.config.custom_rules));
            vuln_matches.extend(vuln_db.scan(&content, &file_name));
            event_issues.extend(analyzer.scan_events(&content));
            unhandled_results.extend(analyzer.scan_unhandled_results(&content));
            upgrade_reports.push(analyzer.analyze_upgrade_patterns(&content));
            smt_issues.extend(analyzer.verify_smt_invariants(&content));
        }
    }

    let total_findings = collisions.len()
        + size_warnings.len()
        + unsafe_patterns.len()
        + auth_gaps.len()
        + panic_issues.len()
        + arithmetic_issues.len()
        + custom_matches.len()
        + event_issues.len()
        + unhandled_results.len()
        + upgrade_reports
            .iter()
            .map(|r| r.findings.len())
            .sum::<usize>()
        + smt_issues.len();

    let has_critical =
        !auth_gaps.is_empty() || panic_issues.iter().any(|p| p.issue_type == "panic!");
    let has_high = !arithmetic_issues.is_empty()
        || !panic_issues.is_empty()
        || !smt_issues.is_empty()
        || !unhandled_results.is_empty()
        || size_warnings
            .iter()
            .any(|w| w.level == SizeWarningLevel::ExceedsLimit);
    let timestamp = chrono_timestamp();

    let webhook_payload = ScanWebhookPayload {
        event: "scan.completed",
        project_path: path.display().to_string(),
        timestamp_unix: timestamp.clone(),
        summary: ScanWebhookSummary {
            total_findings,
            has_critical,
            has_high,
        },
    };

    if let Err(err) = send_scan_completed_webhooks(&args.webhook_urls, &webhook_payload) {
        eprintln!("⚠️ Failed to initialize webhook client: {}", err);
    }

    if is_json {
        let report = serde_json::json!({
            "storage_collisions": collisions,
            "ledger_size_warnings": size_warnings,
            "unsafe_patterns": unsafe_patterns,
            "auth_gaps": auth_gaps,
            "panic_issues": panic_issues,
            "arithmetic_issues": arithmetic_issues,
            "custom_rules": custom_matches,
            "event_issues": event_issues,
            "unhandled_results": unhandled_results,
            "upgrade_reports": upgrade_reports,
            "smt_issues": smt_issues,
            "vulnerability_db_matches": vuln_matches,
            "vulnerability_db_version": vuln_db.version,
            "metadata": {
                "version": env!("CARGO_PKG_VERSION"),
                "timestamp": timestamp,
                "project_path": path.display().to_string(),
                "format": "sanctifier-ci-v1",
            },
            "error_codes": finding_codes::all_finding_codes(),
            "summary": {
                "total_findings": total_findings,
                "storage_collisions": collisions.len(),
                "auth_gaps": auth_gaps.len(),
                "panic_issues": panic_issues.len(),
                "arithmetic_issues": arithmetic_issues.len(),
                "size_warnings": size_warnings.len(),
                "unsafe_patterns": unsafe_patterns.len(),
                "custom_rule_matches": custom_matches.len(),
                "event_issues": event_issues.len(),
                "unhandled_results": unhandled_results.len(),
                "smt_issues": smt_issues.len(),
                "has_critical": has_critical,
                "has_high": has_high,
            },
            "findings": {
                "storage_collisions": collisions.iter().map(|c| serde_json::json!({
                    "code": finding_codes::STORAGE_COLLISION,
                    "key_value": c.key_value,
                    "key_type": c.key_type,
                    "location": c.location,
                    "message": c.message,
                })).collect::<Vec<_>>(),
                "ledger_size_warnings": size_warnings.iter().map(|w| serde_json::json!({
                    "code": finding_codes::LEDGER_SIZE_RISK,
                    "struct_name": w.struct_name,
                    "estimated_size": w.estimated_size,
                    "limit": w.limit,
                    "level": w.level,
                })).collect::<Vec<_>>(),
                "unsafe_patterns": unsafe_patterns.iter().map(|p| serde_json::json!({
                    "code": finding_codes::UNSAFE_PATTERN,
                    "pattern_type": p.pattern_type,
                    "line": p.line,
                    "snippet": p.snippet,
                })).collect::<Vec<_>>(),
                "auth_gaps": auth_gaps.iter().map(|g| serde_json::json!({
                    "code": finding_codes::AUTH_GAP,
                    "function": g,
                })).collect::<Vec<_>>(),
                "panic_issues": panic_issues.iter().map(|p| serde_json::json!({
                    "code": finding_codes::PANIC_USAGE,
                    "function_name": p.function_name,
                    "issue_type": p.issue_type,
                    "location": p.location,
                })).collect::<Vec<_>>(),
                "arithmetic_issues": arithmetic_issues.iter().map(|a| serde_json::json!({
                    "code": finding_codes::ARITHMETIC_OVERFLOW,
                    "function_name": a.function_name,
                    "operation": a.operation,
                    "suggestion": a.suggestion,
                    "location": a.location,
                })).collect::<Vec<_>>(),
                "custom_rules": custom_matches.iter().map(|m| serde_json::json!({
                    "code": finding_codes::CUSTOM_RULE_MATCH,
                    "rule_name": m.rule_name,
                    "line": m.line,
                    "snippet": m.snippet,
                    "severity": m.severity,
                })).collect::<Vec<_>>(),
                "event_issues": event_issues.iter().map(|e| serde_json::json!({
                    "code": finding_codes::EVENT_INCONSISTENCY,
                    "event_name": e.event_name,
                    "issue_type": e.issue_type,
                    "location": e.location,
                    "message": e.message,
                })).collect::<Vec<_>>(),
                "unhandled_results": unhandled_results.iter().map(|r| serde_json::json!({
                    "code": finding_codes::UNHANDLED_RESULT,
                    "function_name": r.function_name,
                    "call_expression": r.call_expression,
                    "location": r.location,
                    "message": r.message,
                })).collect::<Vec<_>>(),
                "upgrade_risks": upgrade_reports.iter().flat_map(|r| &r.findings).map(|f| serde_json::json!({
                    "code": finding_codes::UPGRADE_RISK,
                    "category": f.category,
                    "function_name": f.function_name,
                    "location": f.location,
                    "message": f.message,
                    "suggestion": f.suggestion,
                })).collect::<Vec<_>>(),
                "smt_issues": smt_issues.iter().map(|s| serde_json::json!({
                    "code": finding_codes::SMT_INVARIANT_VIOLATION,
                    "function_name": s.function_name,
                    "description": s.description,
                    "location": s.location,
                })).collect::<Vec<_>>(),
            },
        });
        println!("{}", serde_json::to_string_pretty(&report)?);

        if has_critical || has_high {
            std::process::exit(1);
        }
        return Ok(());
    }

    if collisions.is_empty() {
        println!("\n{} No storage key collisions found.", "✅".green());
    } else {
        println!("{} Static analysis complete.\n", "✅".green());

        let rt = Runtime::new().unwrap();

        if all_size_warnings.is_empty() {
            println!("No ledger size issues found.");
        } else {
            for warning in all_size_warnings {
                println!(
                    "{} Warning: Struct {} is approaching ledger entry size limit!",
                    "⚠️".yellow(),
                    warning.struct_name.bold()
                );
                if args.llm_explain {
                    let detail = format!("Struct {} estimated size {} bytes (limit {})", warning.struct_name, warning.estimated_size, warning.limit);
                    if let Ok(resp) = rt.block_on(llm::get_llm_explanation("ledger_size", &detail)) {
                        println!("      {} {}", "LLM Explanation:".cyan(), resp.explanation);
                        println!("      {} {}", "Mitigation:".cyan(), resp.mitigation);
                    }
                }
            }
        }
    }

        if !all_auth_gaps.is_empty() {
            println!("\n{} Found potential Authentication Gaps!", "🛑".red());
            for gap in all_auth_gaps {
                println!("   {} Function {} is modifying state without require_auth()", "->".red(), gap.bold());
                if args.llm_explain {
                    if let Ok(resp) = rt.block_on(llm::get_llm_explanation("auth_gap", &gap)) {
                        println!("      {} {}", "LLM Explanation:".cyan(), resp.explanation);
                        println!("      {} {}", "Mitigation:".cyan(), resp.mitigation);
                    }
                }
            }
        } else {
            println!("\nNo authentication gaps found.");
        }
    }

    if !event_issues.is_empty() {
        println!(
            "\n{} Found Event Consistency/Optimization issues!",
            "⚠️".yellow()
        );
        for issue in &event_issues {
            println!(
                "   {} [{}] Event: {}",
                "->".red(),
                finding_codes::EVENT_INCONSISTENCY.bold(),
                issue.event_name.bold()
            );
            println!("      Type: {:?}", issue.issue_type);
            println!("      Location: {}", issue.location);
            println!("      Message: {}", issue.message);
        }
    }

    if !unhandled_results.is_empty() {
        println!("\n{} Found Unhandled Result issues!", "⚠️".yellow());
        for issue in &unhandled_results {
            println!(
                "   {} [{}] Function: {}",
                "->".red(),
                finding_codes::UNHANDLED_RESULT.bold(),
                issue.function_name.bold()
            );
            println!("      Call: {}", issue.call_expression);
            println!("      Location: {}", issue.location);
            println!("      Message: {}", issue.message);
        }
    }

    let total_upgrade_findings: usize = upgrade_reports.iter().map(|r| r.findings.len()).sum();
    if total_upgrade_findings > 0 {
        println!("\n{} Found Upgrade/Admin Risk issues!", "⚠️".yellow());
        for report in &upgrade_reports {
            for finding in &report.findings {
                println!(
                    "   {} [{}] Category: {:?}",
                    "->".red(),
                    issue.function_name.bold(),
                    issue.issue_type.yellow().bold(),
                    issue.location
                );
                if args.llm_explain {
                    let detail = format!("Function {}: {} at {}", issue.function_name, issue.issue_type, issue.location);
                    if let Ok(resp) = rt.block_on(llm::get_llm_explanation("panic_issue", &detail)) {
                        println!("      {} {}", "LLM Explanation:".cyan(), resp.explanation);
                        println!("      {} {}", "Mitigation:".cyan(), resp.mitigation);
                    }
                }
            }
        }
    }

        if !all_arithmetic_issues.is_empty() {
            println!("\n{} Found unchecked Arithmetic Operations!", "🔢".yellow());
            for issue in all_arithmetic_issues {
                println!(
                    "   {} Function {}: Unchecked `{}` ({})",
                    "->".red(),
                    issue.function_name.bold(),
                    issue.operation.yellow().bold(),
                    issue.location
                );
                if args.llm_explain {
                    let detail = format!("Function {}: {} at {}", issue.function_name, issue.operation, issue.location);
                    if let Ok(resp) = rt.block_on(llm::get_llm_explanation("arithmetic_issue", &detail)) {
                        println!("      {} {}", "LLM Explanation:".cyan(), resp.explanation);
                        println!("      {} {}", "Mitigation:".cyan(), resp.mitigation);
                    }
                }
            }
        }
    }
    Ok(())
}

fn is_soroban_project(path: &Path) -> bool {
    // Basic heuristics for tests.
    if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        return true;
    }
    let cargo_toml_path = if path.is_dir() {
        path.join("Cargo.toml")
    } else {
        path.to_path_buf()
    };
    cargo_toml_path.exists()
}
