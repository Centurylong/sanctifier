use std::fs;
use std::path::{Path, PathBuf};
use clap::Args;
use colored::*;
use sanctifier_core::{Analyzer, ArithmeticIssue, SizeWarning, UnsafePattern};
use crate::ws_server::{LogEvent, LogSender};

#[derive(Args, Debug)]
pub struct StreamArgs {
    /// Path to the contract directory or Cargo.toml
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// WebSocket server port
    #[arg(short, long, default_value = "9001")]
    pub port: u16,

    /// Limit for ledger entry size in bytes
    #[arg(short, long, default_value = "64000")]
    pub limit: usize,
}

pub async fn exec(args: StreamArgs, log_tx: LogSender) -> anyhow::Result<()> {
    let path = &args.path;

    log_tx.send(LogEvent::info("Starting Sanctifier analysis..."))?;

    if !is_soroban_project(path) {
        let msg = format!("Error: {:?} is not a valid Soroban project", path);
        log_tx.send(LogEvent::error(&msg))?;
        anyhow::bail!(msg);
    }

    log_tx.send(LogEvent::info(format!("Valid Soroban project found at {:?}", path)))?;

    let mut analyzer = Analyzer::new(sanctifier_core::SanctifyConfig::default());
    
    let mut all_size_warnings: Vec<SizeWarning> = Vec::new();
    let mut all_unsafe_patterns: Vec<UnsafePattern> = Vec::new();
    let mut all_auth_gaps: Vec<String> = Vec::new();
    let mut all_panic_issues = Vec::new();
    let mut all_arithmetic_issues: Vec<ArithmeticIssue> = Vec::new();

    if path.is_dir() {
        analyze_directory_with_logging(
            path,
            &analyzer,
            &mut all_size_warnings,
            &mut all_unsafe_patterns,
            &mut all_auth_gaps,
            &mut all_panic_issues,
            &mut all_arithmetic_issues,
            &log_tx,
        )?;
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        log_tx.send(LogEvent::file_analysis(
            path.display().to_string(),
            "analyzing",
        ))?;

        if let Ok(content) = fs::read_to_string(path) {
            all_size_warnings.extend(analyzer.analyze_ledger_size(&content));
            let patterns = analyzer.analyze_unsafe_patterns(&content);
            for mut p in patterns {
                p.snippet = format!("{}: {}", path.display(), p.snippet);
                all_unsafe_patterns.push(p);
            }

            let gaps = analyzer.scan_auth_gaps(&content);
            for g in gaps {
                all_auth_gaps.push(format!("{}: {}", path.display(), g));
            }

            let panics = analyzer.scan_panics(&content);
            for p in panics {
                let mut p_mod = p.clone();
                p_mod.location = format!("{}: {}", path.display(), p.location);
                all_panic_issues.push(p_mod);
            }

            let arith = analyzer.scan_arithmetic_overflow(&content);
            for mut a in arith {
                a.location = format!("{}: {}", path.display(), a.location);
                all_arithmetic_issues.push(a);
            }
        }

        log_tx.send(LogEvent::file_analysis(
            path.display().to_string(),
            "complete",
        ))?;
    }

    log_tx.send(LogEvent::info("Static analysis complete"))?;

    // Send summary
    let total_issues = all_size_warnings.len()
        + all_unsafe_patterns.len()
        + all_auth_gaps.len()
        + all_panic_issues.len()
        + all_arithmetic_issues.len();

    if total_issues == 0 {
        log_tx.send(LogEvent::complete("No issues found! ✨"))?;
    } else {
        log_tx.send(LogEvent::warning(format!(
            "Found {} total issues",
            total_issues
        )))?;

        if !all_size_warnings.is_empty() {
            log_tx.send(LogEvent::warning(format!(
                "Ledger size warnings: {}",
                all_size_warnings.len()
            )))?;
        }

        if !all_auth_gaps.is_empty() {
            log_tx.send(LogEvent::error(format!(
                "Authentication gaps: {}",
                all_auth_gaps.len()
            )))?;
        }

        if !all_panic_issues.is_empty() {
            log_tx.send(LogEvent::error(format!(
                "Panic/unwrap issues: {}",
                all_panic_issues.len()
            )))?;
        }

        if !all_arithmetic_issues.is_empty() {
            log_tx.send(LogEvent::warning(format!(
                "Arithmetic overflow risks: {}",
                all_arithmetic_issues.len()
            )))?;
        }

        log_tx.send(LogEvent::complete("Analysis complete"))?;
    }

    // Send final JSON report
    let output = serde_json::json!({
        "size_warnings": all_size_warnings,
        "unsafe_patterns": all_unsafe_patterns,
        "auth_gaps": all_auth_gaps,
        "panic_issues": all_panic_issues,
        "arithmetic_issues": all_arithmetic_issues,
    });

    log_tx.send(LogEvent::info(format!(
        "Report: {}",
        serde_json::to_string(&output)?
    )))?;

    Ok(())
}

fn is_soroban_project(path: &Path) -> bool {
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

fn analyze_directory_with_logging(
    dir: &Path,
    analyzer: &Analyzer,
    all_size_warnings: &mut Vec<SizeWarning>,
    all_unsafe_patterns: &mut Vec<UnsafePattern>,
    all_auth_gaps: &mut Vec<String>,
    all_panic_issues: &mut Vec<sanctifier_core::PanicIssue>,
    all_arithmetic_issues: &mut Vec<ArithmeticIssue>,
    log_tx: &LogSender,
) -> anyhow::Result<()> {
    let mut files = Vec::new();
    collect_rust_files(dir, &mut files);

    let total = files.len();
    log_tx.send(LogEvent::info(format!("Found {} Rust files to analyze", total)))?;

    for (idx, path) in files.iter().enumerate() {
        log_tx.send(LogEvent::progress(
            format!("Analyzing {}", path.display()),
            idx + 1,
            total,
        ))?;

        log_tx.send(LogEvent::file_analysis(
            path.display().to_string(),
            "analyzing",
        ))?;

        if let Ok(content) = fs::read_to_string(path) {
            all_size_warnings.extend(analyzer.analyze_ledger_size(&content));

            let patterns = analyzer.analyze_unsafe_patterns(&content);
            for mut p in patterns {
                p.snippet = format!("{}: {}", path.display(), p.snippet);
                all_unsafe_patterns.push(p);
            }

            let gaps = analyzer.scan_auth_gaps(&content);
            for g in gaps {
                all_auth_gaps.push(format!("{}: {}", path.display(), g));
            }

            let panics = analyzer.scan_panics(&content);
            for p in panics {
                let mut p_mod = p.clone();
                p_mod.location = format!("{}: {}", path.display(), p.location);
                all_panic_issues.push(p_mod);
            }

            let arith = analyzer.scan_arithmetic_overflow(&content);
            for mut a in arith {
                a.location = format!("{}: {}", path.display(), a.location);
                all_arithmetic_issues.push(a);
            }
        }

        log_tx.send(LogEvent::file_analysis(
            path.display().to_string(),
            "complete",
        ))?;
    }

    Ok(())
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_rust_files(&path, files);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
}
