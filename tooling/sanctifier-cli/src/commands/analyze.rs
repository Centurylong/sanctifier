use crate::commands::webhook::{
    send_scan_completed_webhooks, ScanWebhookPayload, ScanWebhookSummary,
};
use clap::Args;
use colored::*;
use sanctifier_core::baseline::{apply_baseline, load_baseline, BaselineEntry};
use sanctifier_core::finding_codes;
use sanctifier_core::{Analyzer, SanctifyConfig, SizeWarningLevel};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};

use crate::vulndb::{VulnDatabase, VulnMatch};

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

    /// Path to a custom vulnerability database JSON file
    #[arg(long)]
    pub vuln_db: Option<PathBuf>,
    /// Webhook endpoint(s) to notify when scan completes (Discord/Slack/Teams/custom)
    #[arg(long = "webhook-url")]
    pub webhook_urls: Vec<String>,

    /// Ignore .sanctify-baseline.json and report all findings.
    #[arg(long)]
    pub no_baseline: bool,
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

    // ── Inline suppression ───────────────────────────────────────────────────
    let mut inline_suppressions: std::collections::HashMap<String, Vec<(usize, String, String)>> = std::collections::HashMap::new();

    let mut get_suppressions = |file_path: &str| -> Vec<(usize, String, String)> {
        if let Some(supps) = inline_suppressions.get(file_path) {
            return supps.clone();
        }
        let mut supps = Vec::new();
        if let Ok(content) = fs::read_to_string(file_path) {
            for (i, line) in content.lines().enumerate() {
                if let Some(idx) = line.find("// sanctifier-ignore:") {
                    let rest = line[idx + "// sanctifier-ignore:".len()..].trim();
                    if let Some((code, justification)) = rest.split_once('-') {
                        let justification = justification.trim();
                        if justification.is_empty() {
                            if !is_json {
                                eprintln!("{} Warning: Inline suppression missing justification at {}:{}", "⚠️".yellow(), file_path, i + 1);
                            }
                        } else {
                            supps.push((i + 1, code.trim().to_string(), justification.to_string()));
                        }
                    } else {
                        if !is_json {
                            eprintln!("{} Warning: Inline suppression missing justification at {}:{}", "⚠️".yellow(), file_path, i + 1);
                        }
                    }
                }
            }
        }
        inline_suppressions.insert(file_path.to_string(), supps.clone());
        supps
    };

    let extract_line_and_file = |raw_location: &str| -> Option<(String, usize)> {
        let parts: Vec<&str> = raw_location.split(':').collect();
        if parts.len() < 2 {
            return None;
        }
        let file_name = parts[0].to_string();
        for p in parts.iter().skip(1) {
            let p = p.trim_start_matches("line ").trim();
            if let Ok(line) = p.parse::<usize>() {
                return Some((file_name, line));
            }
        }
        None
    };

    let mut is_inline_suppressed = |raw_location: &str, code: &str| -> bool {
        if let Some((file_path, line_num)) = extract_line_and_file(raw_location) {
            let supps = get_suppressions(&file_path);
            for (s_line, s_code, _) in supps {
                if s_code == code && (s_line == &line_num || *s_line + 1 == line_num) {
                    return true;
                }
            }
        }
        false
    };

    auth_gaps.retain(|gap| !is_inline_suppressed(gap, finding_codes::AUTH_GAP));
    panic_issues.retain(|p| !is_inline_suppressed(&p.location, finding_codes::PANIC_USAGE));
    arithmetic_issues.retain(|a| !is_inline_suppressed(&a.location, finding_codes::ARITHMETIC_OVERFLOW));
    collisions.retain(|c| !is_inline_suppressed(&c.location, finding_codes::STORAGE_COLLISION));
    unsafe_patterns.retain(|u| {
        let file_name = u.snippet.split(':').next().unwrap_or("");
        let supps = get_suppressions(file_name);
        !supps.iter().any(|(s_line, s_code, _)| {
            s_code == finding_codes::UNSAFE_PATTERN && (*s_line == u.line || *s_line + 1 == u.line)
        })
    });
    event_issues.retain(|e| !is_inline_suppressed(&e.location, finding_codes::EVENT_INCONSISTENCY));
    unhandled_results.retain(|r| !is_inline_suppressed(&r.location, finding_codes::UNHANDLED_RESULT));
    for rep in &mut upgrade_reports {
        rep.findings.retain(|f| !is_inline_suppressed(&f.location, finding_codes::UPGRADE_RISK));
    }
    smt_issues.retain(|s| !is_inline_suppressed(&s.location, finding_codes::SMT_INVARIANT_VIOLATION));
    custom_matches.retain(|m| {
        let file_name = m.snippet.split(':').next().unwrap_or("");
        let supps = get_suppressions(file_name);
        !supps.iter().any(|(s_line, s_code, _)| {
            s_code == finding_codes::CUSTOM_RULE_MATCH && (*s_line == m.line || *s_line + 1 == m.line)
        })
    });
    vuln_matches.retain(|m| {
        let supps = get_suppressions(&m.file);
        !supps.iter().any(|(s_line, s_code, _)| {
            s_code == m.vuln_id && (*s_line == m.line || *s_line + 1 == m.line)
        })
    });

    // ── Baseline suppression ─────────────────────────────────────────────────
    // Load .sanctify-baseline.json from the project root (if it exists and
    // --no-baseline was not passed) and filter out pre-existing findings.
    let project_root = if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    };

    let (suppressed_count, stale_entries) = if !args.no_baseline {
        match load_baseline(&project_root) {
            Ok(Some(ref bl)) => {
                use sanctifier_core::baseline::FlatFinding;
                use std::collections::HashSet;

                // Build FlatFindings from current typed results without re-running analysis.
                let mut current_flat: Vec<FlatFinding> = Vec::new();

                for gap in &auth_gaps {
                    let ctx = gap.split(':').next_back().unwrap_or(gap.as_str());
                    current_flat.push(FlatFinding::new(finding_codes::AUTH_GAP, gap, ctx));
                }
                for p in &panic_issues {
                    let ctx = format!("{}|{}", p.function_name, p.issue_type);
                    current_flat.push(FlatFinding::new(finding_codes::PANIC_USAGE, &p.location, &ctx));
                }
                for a in &arithmetic_issues {
                    let ctx = format!("{}|{}", a.function_name, a.operation);
                    current_flat.push(FlatFinding::new(finding_codes::ARITHMETIC_OVERFLOW, &a.location, &ctx));
                }
                for w in &size_warnings {
                    // Size warnings have no per-file location in the typed struct; use key only.
                    current_flat.push(FlatFinding::new(finding_codes::LEDGER_SIZE_RISK, "", &w.struct_name));
                }
                for c in &collisions {
                    let ctx = format!("{}|{}", c.key_value, c.key_type);
                    current_flat.push(FlatFinding::new(finding_codes::STORAGE_COLLISION, &c.location, &ctx));
                }
                for u in &unsafe_patterns {
                    let ctx = format!("{:?}", u.pattern_type);
                    current_flat.push(FlatFinding::new(finding_codes::UNSAFE_PATTERN, "", &ctx));
                }
                for e in &event_issues {
                    let ctx = format!("{}|{:?}", e.event_name, e.issue_type);
                    current_flat.push(FlatFinding::new(finding_codes::EVENT_INCONSISTENCY, &e.location, &ctx));
                }
                for r in &unhandled_results {
                    let ctx = format!("{}|{}", r.function_name, r.call_expression);
                    current_flat.push(FlatFinding::new(finding_codes::UNHANDLED_RESULT, &r.location, &ctx));
                }
                for rep in &upgrade_reports {
                    for f in &rep.findings {
                        let ctx = format!("{:?}|{}", f.category, f.function_name.as_deref().unwrap_or(""));
                        current_flat.push(FlatFinding::new(finding_codes::UPGRADE_RISK, &f.location, &ctx));
                    }
                }
                for s in &smt_issues {
                    let ctx = format!("{}|{}", s.function_name, s.description);
                    current_flat.push(FlatFinding::new(finding_codes::SMT_INVARIANT_VIOLATION, &s.location, &ctx));
                }

                let (new_flat, stale) = apply_baseline(bl, &current_flat);
                let new_fps: HashSet<String> = new_flat.iter().map(|f| f.fingerprint()).collect();

                // Build suppressed set.
                let suppressed_fps: HashSet<String> = current_flat
                    .iter()
                    .map(|f| f.fingerprint())
                    .filter(|fp| !new_fps.contains(fp))
                    .collect();

                let suppressed_count = suppressed_fps.len();
                let stale_entries: Vec<BaselineEntry> = stale.into_iter().cloned().collect();

                // Filter each typed collection in-place.
                auth_gaps.retain(|gap| {
                    let ctx = gap.split(':').next_back().unwrap_or(gap.as_str());
                    let fp = FlatFinding::new(finding_codes::AUTH_GAP, gap, ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                panic_issues.retain(|p| {
                    let ctx = format!("{}|{}", p.function_name, p.issue_type);
                    let fp = FlatFinding::new(finding_codes::PANIC_USAGE, &p.location, &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                arithmetic_issues.retain(|a| {
                    let ctx = format!("{}|{}", a.function_name, a.operation);
                    let fp = FlatFinding::new(finding_codes::ARITHMETIC_OVERFLOW, &a.location, &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                size_warnings.retain(|w| {
                    let fp = FlatFinding::new(finding_codes::LEDGER_SIZE_RISK, "", &w.struct_name).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                collisions.retain(|c| {
                    let ctx = format!("{}|{}", c.key_value, c.key_type);
                    let fp = FlatFinding::new(finding_codes::STORAGE_COLLISION, &c.location, &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                unsafe_patterns.retain(|u| {
                    let ctx = format!("{:?}", u.pattern_type);
                    let fp = FlatFinding::new(finding_codes::UNSAFE_PATTERN, "", &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                event_issues.retain(|e| {
                    let ctx = format!("{}|{:?}", e.event_name, e.issue_type);
                    let fp = FlatFinding::new(finding_codes::EVENT_INCONSISTENCY, &e.location, &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                unhandled_results.retain(|r| {
                    let ctx = format!("{}|{}", r.function_name, r.call_expression);
                    let fp = FlatFinding::new(finding_codes::UNHANDLED_RESULT, &r.location, &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });
                for rep in &mut upgrade_reports {
                    rep.findings.retain(|f| {
                        let ctx = format!("{:?}|{}", f.category, f.function_name.as_deref().unwrap_or(""));
                        let fp = FlatFinding::new(finding_codes::UPGRADE_RISK, &f.location, &ctx).fingerprint();
                        !suppressed_fps.contains(&fp)
                    });
                }
                smt_issues.retain(|s| {
                    let ctx = format!("{}|{}", s.function_name, s.description);
                    let fp = FlatFinding::new(finding_codes::SMT_INVARIANT_VIOLATION, &s.location, &ctx).fingerprint();
                    !suppressed_fps.contains(&fp)
                });

                (suppressed_count, stale_entries)
            }
            Ok(None) => (0, vec![]),
            Err(e) => {
                if !is_json {
                    eprintln!("{} Could not read baseline: {}", "⚠️".yellow(), e);
                }
                (0, vec![])
            }
        }
    } else {
        (0, vec![])
    };

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

    // ── Baseline summary (text mode) ─────────────────────────────────────────
    if !is_json && suppressed_count > 0 {
        println!(
            "{} {} finding{} suppressed by baseline (run {} to see all)",
            "ℹ️".blue(),
            suppressed_count,
            if suppressed_count == 1 { "" } else { "s" },
            "sanctifier analyze --no-baseline".bold(),
        );
    }
    if !is_json && !stale_entries.is_empty() {
        println!(
            "{} {} stale baseline entr{} (no longer present in the codebase):",
            "ℹ️".blue(),
            stale_entries.len(),
            if stale_entries.len() == 1 { "y" } else { "ies" },
        );
        for e in &stale_entries {
            println!("   {} [{}] {} — {}", "~".yellow(), e.code.bold(), e.path, e.context);
        }
        println!(
            "    Run {} to remove them.",
            "sanctifier baseline --update".bold()
        );
    }

    if is_json {
        let stale_json: Vec<serde_json::Value> = stale_entries
            .iter()
            .map(|e| serde_json::json!({ "fingerprint": e.fingerprint, "code": e.code, "path": e.path, "context": e.context }))
            .collect();
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
            "baseline": {
                "suppressed_count": suppressed_count,
                "stale_entries": stale_json,
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
        println!(
            "\n{} Found potential Storage Key Collisions!",
            "⚠️".yellow()
        );
        for collision in collisions {
            println!(
                "   {} [{}] Value: {}",
                "->".red(),
                finding_codes::STORAGE_COLLISION.bold(),
                collision.key_value.bold()
            );
            println!("      Type: {}", collision.key_type);
            println!("      Location: {}", collision.location);
            println!("      Message: {}", collision.message);
        }
    }

    if auth_gaps.is_empty() {
        println!("{} No authentication gaps found.", "✅".green());
    } else {
        println!("\n{} Found potential Authentication Gaps!", "⚠️".yellow());
        for gap in auth_gaps {
            println!(
                "   {} [{}] Function: {}",
                "->".red(),
                finding_codes::AUTH_GAP.bold(),
                gap.bold()
            );
        }
    }

    if panic_issues.is_empty() {
        println!("{} No explicit Panics/Unwraps found.", "✅".green());
    } else {
        println!("\n{} Found explicit Panics/Unwraps!", "⚠️".yellow());
        for issue in panic_issues {
            println!(
                "   {} [{}] Type: {}",
                "->".red(),
                finding_codes::PANIC_USAGE.bold(),
                issue.issue_type.bold()
            );
            println!("      Location: {}", issue.location);
        }
    }

    if arithmetic_issues.is_empty() {
        println!("{} No unchecked Arithmetic Operations found.", "✅".green());
    } else {
        println!("\n{} Found unchecked Arithmetic Operations!", "⚠️".yellow());
        for issue in arithmetic_issues {
            println!(
                "   {} [{}] Op: {}",
                "->".red(),
                finding_codes::ARITHMETIC_OVERFLOW.bold(),
                issue.operation.bold()
            );
            println!("      Location: {}", issue.location);
        }
    }

    if size_warnings.is_empty() {
        println!("{} No ledger size issues found.", "✅".green());
    } else {
        println!("\n{} Found Ledger Size Warnings!", "⚠️".yellow());
        for warning in size_warnings {
            println!(
                "   {} [{}] Struct: {}",
                "->".red(),
                finding_codes::LEDGER_SIZE_RISK.bold(),
                warning.struct_name.bold()
            );
            println!("      Size: {} bytes", warning.estimated_size);
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
                    finding_codes::UPGRADE_RISK.bold(),
                    finding.category
                );
                if let Some(f_name) = &finding.function_name {
                    println!("      Function: {}", f_name);
                }
                println!("      Location: {}", finding.location);
                println!("      Message: {}", finding.message);
                println!("      Suggestion: {}", finding.suggestion);
            }
        }
    }

    if !smt_issues.is_empty() {
        println!("\n{} Found Formal Verification (SMT) issues!", "❌".red());
        for issue in &smt_issues {
            println!(
                "   {} [{}] Function: {}",
                "->".red(),
                finding_codes::SMT_INVARIANT_VIOLATION.bold(),
                issue.function_name.bold()
            );
            println!("      Description: {}", issue.description);
            println!("      Location: {}", issue.location);
        }
    }

    // Vulnerability database matches
    if vuln_matches.is_empty() {
        println!(
            "{} No known vulnerability patterns matched (DB v{}).",
            "✅".green(),
            vuln_db.version
        );
    } else {
        println!(
            "\n{} Found {} known vulnerability pattern(s) (DB v{})!",
            "🛡️".red(),
            vuln_matches.len(),
            vuln_db.version
        );
        for m in &vuln_matches {
            let sev_icon = match m.severity.as_str() {
                "critical" => "❌".red(),
                "high" => "🔴".red(),
                "medium" => "⚠️".yellow(),
                _ => "ℹ️".blue(),
            };
            println!(
                "   {} [{}] {} ({})",
                sev_icon,
                m.vuln_id.bold(),
                m.name.bold(),
                m.severity.to_uppercase()
            );
            println!("      File: {}:{}", m.file, m.line);
            println!("      {}", m.description);
            println!("      Suggestion: {}", m.recommendation);
        }
    }

    println!("\n{} Static analysis complete.", "✨".green());

    Ok(())
}

fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let duration = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    format!("{}", secs)
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

#[allow(clippy::too_many_arguments)]
fn walk_dir(
    dir: &Path,
    analyzer: &Analyzer,
    vuln_db: &VulnDatabase,
    collisions: &mut Vec<sanctifier_core::StorageCollisionIssue>,
    size_warnings: &mut Vec<sanctifier_core::SizeWarning>,
    unsafe_patterns: &mut Vec<sanctifier_core::UnsafePattern>,
    auth_gaps: &mut Vec<String>,
    panic_issues: &mut Vec<sanctifier_core::PanicIssue>,
    arithmetic_issues: &mut Vec<sanctifier_core::ArithmeticIssue>,
    custom_matches: &mut Vec<sanctifier_core::CustomRuleMatch>,
    vuln_matches: &mut Vec<VulnMatch>,
    event_issues: &mut Vec<sanctifier_core::EventIssue>,
    unhandled_results: &mut Vec<sanctifier_core::UnhandledResultIssue>,
    upgrade_reports: &mut Vec<sanctifier_core::UpgradeReport>,
    smt_issues: &mut Vec<sanctifier_core::smt::SmtInvariantIssue>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip ignore_paths
            let is_ignored = analyzer
                .config
                .ignore_paths
                .iter()
                .any(|p| path.ends_with(p));
            if is_ignored {
                continue;
            }

            walk_dir(
                &path,
                analyzer,
                vuln_db,
                collisions,
                size_warnings,
                unsafe_patterns,
                auth_gaps,
                panic_issues,
                arithmetic_issues,
                custom_matches,
                vuln_matches,
                event_issues,
                unhandled_results,
                upgrade_reports,
                smt_issues,
            )?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = fs::read_to_string(&path) {
                let file_name = path.display().to_string();

                let mut c = analyzer.scan_storage_collisions(&content);
                for i in &mut c {
                    i.location = format!("{}:{}", file_name, i.location);
                }
                collisions.extend(c);

                let s = analyzer.analyze_ledger_size(&content);
                size_warnings.extend(s);

                let mut u = analyzer.analyze_unsafe_patterns(&content);
                for i in &mut u {
                    i.snippet = format!("{}:{}", file_name, i.snippet);
                }
                unsafe_patterns.extend(u);

                for g in analyzer.scan_auth_gaps(&content) {
                    auth_gaps.push(format!("{}:{}", file_name, g));
                }

                let mut p = analyzer.scan_panics(&content);
                for i in &mut p {
                    i.location = format!("{}:{}", file_name, i.location);
                    panic_issues.push(i.clone());
                }

                let mut a = analyzer.scan_arithmetic_overflow(&content);
                for i in &mut a {
                    i.location = format!("{}:{}", file_name, i.location);
                    arithmetic_issues.push(i.clone());
                }

                let mut custom =
                    analyzer.analyze_custom_rules(&content, &analyzer.config.custom_rules);
                for m in &mut custom {
                    m.snippet = format!("{}:{}: {}", file_name, m.line, m.snippet);
                }
                custom_matches.extend(custom);

                // Scan against vulnerability database
                vuln_matches.extend(vuln_db.scan(&content, &file_name));

                let mut e = analyzer.scan_events(&content);
                for i in &mut e {
                    i.location = format!("{}:{}", file_name, i.location);
                }
                event_issues.extend(e);

                let mut r = analyzer.scan_unhandled_results(&content);
                for i in &mut r {
                    i.location = format!("{}:{}", file_name, i.location);
                }
                unhandled_results.extend(r);

                let mut up = analyzer.analyze_upgrade_patterns(&content);
                for f in &mut up.findings {
                    f.location = format!("{}:{}", file_name, f.location);
                }
                upgrade_reports.push(up);

                let mut smt = analyzer.verify_smt_invariants(&content);
                for i in &mut smt {
                    i.location = format!("{}:{}", file_name, i.location);
                }
                smt_issues.extend(smt);
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
