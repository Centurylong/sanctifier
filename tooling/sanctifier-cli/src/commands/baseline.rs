//! `sanctifier baseline [--update]`
//!
//! Snapshot current findings into `.sanctify-baseline.json` so that
//! subsequent `sanctifier analyze` runs only report NEW findings.
//!
//! Use `--update` to refresh the baseline after you have intentionally
//! fixed or accepted some findings.

use clap::Args;
use colored::*;
use sanctifier_core::baseline::{save_baseline, BaselineEntry, FlatFinding, BASELINE_FILE};
use sanctifier_core::finding_codes;
use sanctifier_core::{Analyzer, SanctifyConfig};
use std::fs;
use std::path::{Path, PathBuf};

use crate::vulndb::VulnDatabase;

#[derive(Args, Debug)]
pub struct BaselineArgs {
    /// Path to the contract directory, workspace, or a single `.rs` file.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Overwrite an existing `.sanctify-baseline.json` (refresh after intentional changes).
    #[arg(long)]
    pub update: bool,

    /// Quiet — only print the path of the written file (useful in CI).
    #[arg(short, long)]
    pub quiet: bool,
}

pub fn exec(args: BaselineArgs) -> anyhow::Result<()> {
    let path = &args.path;

    // Determine the project root for writing the baseline file.
    let project_root = if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    };

    let baseline_path = project_root.join(BASELINE_FILE);

    if baseline_path.exists() && !args.update {
        eprintln!(
            "{} {} already exists. Use {} to overwrite.",
            "⚠️".yellow(),
            BASELINE_FILE,
            "--update".bold()
        );
        eprintln!(
            "    Run {} to refresh after accepting findings.",
            "sanctifier baseline --update".bold()
        );
        std::process::exit(1);
    }

    if !args.quiet {
        println!("{} Running analysis to collect current findings…", "🔍".blue());
    }

    let config = load_config(path);
    let analyzer = Analyzer::new(config.clone());
    let vuln_db = VulnDatabase::load_default();

    let flat = collect_flat_findings(path, &analyzer, &vuln_db, &config)?;
    let entries: Vec<BaselineEntry> = flat.iter().map(BaselineEntry::from_flat).collect();
    let count = entries.len();

    save_baseline(&project_root, entries)?;

    if args.quiet {
        println!("{}", baseline_path.display());
    } else if count == 0 {
        println!(
            "{} No findings — wrote empty baseline to {}",
            "✅".green(),
            BASELINE_FILE
        );
    } else {
        println!(
            "{} Wrote {} finding{} to {}",
            "✅".green(),
            count,
            if count == 1 { "" } else { "s" },
            BASELINE_FILE.bold()
        );
        println!(
            "    Future {} runs will suppress these and only report new ones.",
            "sanctifier analyze".bold()
        );
    }

    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Collect all findings from the given path and flatten them to `FlatFinding`.
pub(crate) fn collect_flat_findings(
    path: &Path,
    analyzer: &Analyzer,
    vuln_db: &VulnDatabase,
    _config: &SanctifyConfig,
) -> anyhow::Result<Vec<FlatFinding>> {
    let mut flat: Vec<FlatFinding> = Vec::new();

    if path.is_dir() {
        collect_dir(path, analyzer, vuln_db, &mut flat)?;
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        let content = fs::read_to_string(path)?;
        let file_name = path.display().to_string();
        flatten_file_findings(&content, &file_name, analyzer, vuln_db, &mut flat);
    }

    Ok(flat)
}

fn collect_dir(
    dir: &Path,
    analyzer: &Analyzer,
    vuln_db: &VulnDatabase,
    flat: &mut Vec<FlatFinding>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let child = entry.path();
        if child.is_dir() {
            let is_ignored = analyzer
                .config
                .ignore_paths
                .iter()
                .any(|p| child.ends_with(p));
            if is_ignored {
                continue;
            }
            collect_dir(&child, analyzer, vuln_db, flat)?;
        } else if child.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = fs::read_to_string(&child) {
                let file_name = child.display().to_string();
                flatten_file_findings(&content, &file_name, analyzer, vuln_db, flat);
            }
        }
    }
    Ok(())
}

fn flatten_file_findings(
    content: &str,
    file_name: &str,
    analyzer: &Analyzer,
    vuln_db: &VulnDatabase,
    flat: &mut Vec<FlatFinding>,
) {
    // S001 — Auth gaps
    for gap in analyzer.scan_auth_gaps(content) {
        // gap is the function name
        let loc = format!("{file_name}:{gap}");
        flat.push(FlatFinding::new(finding_codes::AUTH_GAP, &loc, &gap));
    }

    // S002 — Panic issues
    for p in analyzer.scan_panics(content) {
        let loc = format!("{file_name}:{}", p.location);
        let ctx = format!("{}|{}", p.function_name, p.issue_type);
        flat.push(FlatFinding::new(finding_codes::PANIC_USAGE, &loc, &ctx));
    }

    // S003 — Arithmetic overflow
    for a in analyzer.scan_arithmetic_overflow(content) {
        let loc = format!("{file_name}:{}", a.location);
        let ctx = format!("{}|{}", a.function_name, a.operation);
        flat.push(FlatFinding::new(
            finding_codes::ARITHMETIC_OVERFLOW,
            &loc,
            &ctx,
        ));
    }

    // S004 — Ledger size (no line-stable location; struct name alone identifies it)
    for w in analyzer.analyze_ledger_size(content) {
        flat.push(FlatFinding::new(
            finding_codes::LEDGER_SIZE_RISK,
            "",
            &w.struct_name,
        ));
    }

    // S005 — Storage collisions
    for c in analyzer.scan_storage_collisions(content) {
        let loc = format!("{file_name}:{}", c.location);
        let ctx = format!("{}|{}", c.key_value, c.key_type);
        flat.push(FlatFinding::new(
            finding_codes::STORAGE_COLLISION,
            &loc,
            &ctx,
        ));
    }

    // S006 — Unsafe patterns (no stable per-instance location; pattern_type identifies it)
    for u in analyzer.analyze_unsafe_patterns(content) {
        let ctx = format!("{:?}", u.pattern_type);
        flat.push(FlatFinding::new(finding_codes::UNSAFE_PATTERN, "", &ctx));
    }

    // S008 — Event issues
    for e in analyzer.scan_events(content) {
        let loc = format!("{file_name}:{}", e.location);
        let ctx = format!("{}|{:?}", e.event_name, e.issue_type);
        flat.push(FlatFinding::new(
            finding_codes::EVENT_INCONSISTENCY,
            &loc,
            &ctx,
        ));
    }

    // S009 — Unhandled results
    for r in analyzer.scan_unhandled_results(content) {
        let loc = format!("{file_name}:{}", r.location);
        let ctx = format!("{}|{}", r.function_name, r.call_expression);
        flat.push(FlatFinding::new(
            finding_codes::UNHANDLED_RESULT,
            &loc,
            &ctx,
        ));
    }

    // S010 — Upgrade risks
    let up = analyzer.analyze_upgrade_patterns(content);
    for f in &up.findings {
        let loc = format!("{file_name}:{}", f.location);
        let ctx = format!(
            "{:?}|{}",
            f.category,
            f.function_name.as_deref().unwrap_or("")
        );
        flat.push(FlatFinding::new(finding_codes::UPGRADE_RISK, &loc, &ctx));
    }

    // S011 — SMT invariant violations
    for s in analyzer.verify_smt_invariants(content) {
        let loc = format!("{file_name}:{}", s.location);
        let ctx = format!("{}|{}", s.function_name, s.description);
        flat.push(FlatFinding::new(
            finding_codes::SMT_INVARIANT_VIOLATION,
            &loc,
            &ctx,
        ));
    }

    // VulnDB matches
    for m in vuln_db.scan(content, file_name) {
        let loc = format!("{file_name}:{}", m.line);
        let ctx = format!("{}|{}", m.vuln_id, m.name);
        flat.push(FlatFinding::new("VULN", &loc, &ctx));
    }
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
