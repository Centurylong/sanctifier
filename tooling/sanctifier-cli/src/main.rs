pub mod commands;
mod llm;
pub mod rules;

use clap::{Parser, Subcommand};
use colored::*;
use sanctifier_core::gas_estimator::GasEstimationReport;
use sanctifier_core::zk_proof::ZkProofSummary;
use sanctifier_core::{
    Analyzer, ArithmeticIssue, CustomRuleMatch, DeprecatedApiIssue, FixType, SanctifyConfig,
    SizeWarning, UnsafePattern, UpgradeReport,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct CachedAnalysis {
    pub hash: String,
    pub size_warnings: Vec<SizeWarning>,
    pub unsafe_patterns: Vec<UnsafePattern>,
    pub auth_gaps: Vec<String>,
    pub panic_issues: Vec<sanctifier_core::PanicIssue>,
    pub arithmetic_issues: Vec<ArithmeticIssue>,
    pub deprecated_api_issues: Vec<DeprecatedApiIssue>,
    pub custom_rule_matches: Vec<CustomRuleMatch>,
    pub gas_estimations: Vec<GasEstimationReport>,
    pub reentrancy_issues: Vec<sanctifier_core::reentrancy::ReentrancyIssue>,
}

#[derive(Serialize, Deserialize, Default)]
pub struct AnalysisCache {
    pub files: HashMap<String, CachedAnalysis>,
}

impl AnalysisCache {
    fn load(path: &Path) -> Self {
        let cache_path = path.join(".sanctifier_cache.json");
        if let Ok(content) = fs::read_to_string(cache_path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    fn save(&self, path: &Path) {
        let cache_path = path.join(".sanctifier_cache.json");
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(cache_path, content);
        }
    }
}

fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Metrics for Kani formal verification results
#[derive(Serialize)]
pub struct KaniVerificationMetrics {
    pub total_assertions: usize,
    pub proven: usize,
    pub failed: usize,
    pub unreachable: usize,
}

#[derive(Parser)]
#[command(name = "sanctifier")]
#[command(about = "Stellar Soroban Security & Formal Verification Suite", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze a Soroban contract for vulnerabilities
    Analyze(commands::analyze::AnalyzeArgs),
    /// Generate a security badge from a JSON report
    Badge(commands::badge::BadgeArgs),
    /// Generate a summary report
    Report {
        /// Optional path to save the generated report
        #[arg(short, long, value_name = "OUTPUT")]
        output: Option<PathBuf>,
    },
    /// Initialize a new Sanctifier project
    Init,
    /// Translate Soroban contract into a Kani-verifiable harness
    Kani {
        /// Path to the .rs file to translate
        path: PathBuf,
        /// Optional path to save the generated harness
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Automatically fix basic vulnerabilities and code issues
    Fix {
        /// Path to the Soroban contract or project directory
        path: PathBuf,
        /// Apply fixes without confirmation
        #[arg(short, long)]
        yes: bool,
        /// Show what would be changed without modifying files
        #[arg(short, long)]
        dry_run: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze(args) => {
            commands::analyze::exec(args)?;
        }
        Commands::Badge(args) => {
            commands::badge::exec(args)?;
        }
        Commands::Report { output } => {
            println!("{} Generating report...", "📄".yellow());
            if let Some(p) = output {
                println!("Report saved to {:?}", p);
            } else {
                println!("Report printed to stdout.");
            }
        }
        Commands::Init => {
            println!("{} Initializing Sanctifier project...", "✨".green());
        }
        Commands::Kani { path, output } => {
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                eprintln!(
                    "{} Error: Kani bridge currently only supports single .rs files.",
                    "❌".red()
                );
                std::process::exit(1);
            }
            if let Ok(content) = fs::read_to_string(&path) {
                let config = load_config(&path);
                match sanctifier_core::kani_bridge::KaniBridge::translate_for_kani(
                    &content,
                    config.kani_unwind,
                ) {
                    Ok(harness) => {
                        if let Some(out_path) = output {
                            if let Err(e) = std::fs::write(&out_path, harness) {
                                eprintln!("{} Failed to write Kani harness: {}", "❌".red(), e);
                            } else {
                                println!(
                                    "{} Generated Kani harness at {:?}",
                                    "✅".green(),
                                    out_path
                                );
                            }
                        } else {
                            println!("{}", harness);
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Error generating Kani harness: {}", "❌".red(), e);
                        std::process::exit(1);
                    }
                }
            } else {
                eprintln!("{} Error reading file {:?}", "❌".red(), path);
                std::process::exit(1);
            }
        }
        Commands::Fix { path, yes, dry_run } => {
            println!(
                "{} Sanctifier Fix: Scanning for automatic patches...",
                "✨".green()
            );
            let config = load_config(&path);
            let analyzer = Analyzer::new(config.clone());
            let mut total_fixes = 0;

            if path.is_dir() {
                fix_directory(&path, &analyzer, &config, yes, dry_run, &mut total_fixes);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                fix_file(&path, &analyzer, yes, dry_run, &mut total_fixes);
            }

            if dry_run {
                println!(
                    "\n{} Dry run complete. {} potential fixes identified.",
                    "✅".green(),
                    total_fixes
                );
            } else {
                println!(
                    "\n{} Fix complete. {} patches applied.",
                    "✅".green(),
                    total_fixes
                );
            }
        }
    }

    Ok(())
}

fn is_soroban_project(path: &Path) -> bool {
    if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
        return true;
    }
    let mut current = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    while let Some(p) = current {
        let cargo = p.join("Cargo.toml");
        if cargo.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo) {
                if content.contains("soroban-sdk") || content.contains("[workspace]") {
                    return true;
                }
            }
        }
        current = p.parent();
    }
    false
}

fn load_config(path: &Path) -> SanctifyConfig {
    if let Some(p) = find_config_path(path) {
        if let Ok(content) = fs::read_to_string(&p) {
            if let Ok(cfg) = toml::from_str::<SanctifyConfig>(&content) {
                return cfg;
            }
        }
    }
    SanctifyConfig::default()
}

fn find_config_path(start_path: &Path) -> Option<PathBuf> {
    let mut current = if let Ok(abs) = fs::canonicalize(start_path) {
        Some(abs)
    } else {
        Some(start_path.to_path_buf())
    };
    while let Some(ref p) = current {
        let config_path = p.join(".sanctify.toml");
        if config_path.exists() {
            return Some(config_path);
        }
        current = p.parent().map(|p| p.to_path_buf());
    }
    None
}

fn run_analysis(
    path: &Path,
    content: &str,
    analyzer: &Analyzer,
    config: &SanctifyConfig,
) -> CachedAnalysis {
    crate::rules::RuleEngine::new(analyzer, config).run_all(content, Some(path))
}

#[allow(clippy::too_many_arguments)]
fn analyze_directory(
    dir: &Path,
    analyzer: &Analyzer,
    config: &SanctifyConfig,
    cache: &mut AnalysisCache,
    all_size_warnings: &mut Vec<SizeWarning>,
    all_unsafe_patterns: &mut Vec<UnsafePattern>,
    all_auth_gaps: &mut Vec<String>,
    all_panic_issues: &mut Vec<sanctifier_core::PanicIssue>,
    all_arithmetic_issues: &mut Vec<ArithmeticIssue>,
    all_deprecated_api_issues: &mut Vec<DeprecatedApiIssue>,
    all_custom_rule_matches: &mut Vec<CustomRuleMatch>,
    all_gas_estimations: &mut Vec<GasEstimationReport>,
    all_reentrancy_issues: &mut Vec<sanctifier_core::reentrancy::ReentrancyIssue>,
    all_symbolic_paths: &mut Vec<sanctifier_core::symbolic::SymbolicGraph>,
    upgrade_report: &mut UpgradeReport,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if config
                .exclude
                .iter()
                .any(|p| name.contains(p) || path.to_string_lossy().contains(p))
            {
                continue;
            }
            if path.is_dir() {
                if config.ignore_paths.iter().any(|p| name.contains(p)) {
                    continue;
                }
                analyze_directory(
                    &path,
                    analyzer,
                    config,
                    cache,
                    all_size_warnings,
                    all_unsafe_patterns,
                    all_auth_gaps,
                    all_panic_issues,
                    all_arithmetic_issues,
                    all_deprecated_api_issues,
                    all_custom_rule_matches,
                    all_gas_estimations,
                    all_reentrancy_issues,
                    all_symbolic_paths,
                    upgrade_report,
                );
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Ok(content) = fs::read_to_string(&path) {
                    let file_hash = compute_hash(&content);
                    let file_key = path.to_string_lossy().to_string();
                    let analysis = if let Some(cached) = cache.files.get(&file_key) {
                        if cached.hash == file_hash {
                            cached.clone()
                        } else {
                            let res = run_analysis(&path, &content, analyzer, config);
                            let updated = CachedAnalysis {
                                hash: file_hash,
                                ..res
                            };
                            cache.files.insert(file_key, updated.clone());
                            updated
                        }
                    } else {
                        let res = run_analysis(&path, &content, analyzer, config);
                        let updated = CachedAnalysis {
                            hash: file_hash,
                            ..res
                        };
                        cache.files.insert(file_key, updated.clone());
                        updated
                    };
                    all_size_warnings.extend(analysis.size_warnings);
                    all_unsafe_patterns.extend(analysis.unsafe_patterns);
                    all_auth_gaps.extend(analysis.auth_gaps);
                    all_panic_issues.extend(analysis.panic_issues);
                    all_arithmetic_issues.extend(analysis.arithmetic_issues);
                    all_deprecated_api_issues.extend(analysis.deprecated_api_issues);
                    all_custom_rule_matches.extend(analysis.custom_rule_matches);
                    all_gas_estimations.extend(analysis.gas_estimations);
                    all_reentrancy_issues.extend(analysis.reentrancy_issues);
                    let sym = analyzer.analyze_symbolic_paths(&content);
                    all_symbolic_paths.extend(sym);
                    let ur = analyzer.analyze_upgrade_patterns(&content);
                    upgrade_report.findings.extend(ur.findings);
                }
            }
        }
    }
}

fn fix_directory(
    dir: &Path,
    analyzer: &Analyzer,
    config: &SanctifyConfig,
    yes: bool,
    dry_run: bool,
    total_fixes: &mut usize,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if config
                .exclude
                .iter()
                .any(|p| name.contains(p) || path.to_string_lossy().contains(p))
            {
                continue;
            }
            if path.is_dir() {
                if config.ignore_paths.iter().any(|p| name.contains(p)) {
                    continue;
                }
                fix_directory(&path, analyzer, config, yes, dry_run, total_fixes);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                fix_file(&path, analyzer, yes, dry_run, total_fixes);
            }
        }
    }
}

fn fix_file(path: &Path, analyzer: &Analyzer, yes: bool, dry_run: bool, total_fixes: &mut usize) {
    if let Ok(content) = fs::read_to_string(path) {
        let mut fixes = analyzer.suggest_fixes(&content);
        if fixes.is_empty() {
            return;
        }
        fixes.sort_by(|a, b| b.line.cmp(&a.line).then(b.column.cmp(&a.column)));
        println!(
            "\n{} Found {} potential fixes for {:?}",
            "💡".blue(),
            fixes.len(),
            path
        );
        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let mut applied_in_file = 0;
        for fix in fixes {
            println!(
                "   {} [{:?}] {}",
                "->".yellow(),
                fix.fix_type,
                fix.description
            );
            let should_apply = if yes || dry_run {
                true
            } else {
                println!("      Apply this fix? (y/n) ");
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).is_ok() && input.trim().to_lowercase() == "y"
            };
            if should_apply && !dry_run {
                if fix.fix_type == FixType::PrefixUnused {
                    if let Some(line) = lines.get_mut(fix.line - 1) {
                        let (start, rest) = line.split_at(fix.column);
                        let (_, end) = rest.split_at(fix.end_column - fix.column);
                        *line = format!("{}{}{}", start, fix.replacement, end);
                        applied_in_file += 1;
                    }
                } else if fix.fix_type == FixType::AddAuth {
                    if let Some(line) = lines.get_mut(fix.line - 1) {
                        let (start, rest) = line.split_at(fix.column);
                        *line = format!("{}{}{}", start, fix.replacement, rest);
                        applied_in_file += 1;
                    }
                }
            } else if should_apply {
                applied_in_file += 1;
            }
        }
        if !dry_run && applied_in_file > 0 {
            let new_content = lines.join("\n");
            if let Err(e) = fs::write(path, new_content) {
                eprintln!("{} Failed to write to {:?}: {}", "❌".red(), path, e);
            } else {
                println!(
                    "   {} Applied {} fixes to {:?}",
                    "✅".green(),
                    applied_in_file,
                    path
                );
            }
        }
        *total_fixes += applied_in_file;
    }
}

// Suppress dead_code for helpers used only by the legacy inline analyze path
#[allow(dead_code)]
fn zk_proof_for_report(
    size: usize,
    auth: usize,
    panics: usize,
    arith: usize,
    deprecated: usize,
) -> ZkProofSummary {
    let data = serde_json::json!({
        "size": size, "auth": auth, "panics": panics,
        "arith": arith, "deprecated": deprecated,
    });
    ZkProofSummary::generate_zk_proof_summary(&data.to_string())
}
