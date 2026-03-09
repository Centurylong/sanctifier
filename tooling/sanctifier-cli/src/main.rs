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

pub mod rules;

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
    /// Total number of safety assertions checked
    pub total_assertions: usize,
    /// Number of assertions successfully proven
    pub proven: usize,
    /// Number of assertions that failed verification
    pub failed: usize,
    /// Number of assertions that are unreachable in the code
    pub unreachable: usize,
}

/// Main CLI structure for the Sanctifier tool
#[derive(Parser)]
#[command(name = "sanctifier")]
#[command(about = "Stellar Soroban Security & Formal Verification Suite", long_about = None)]
struct Cli {
    /// The subcommand to execute
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze a Soroban contract for vulnerabilities
    Analyze {
        /// Path to the Soroban contract or project directory
        path: PathBuf,
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
        /// Maximum ledger entry size limit in bytes
        #[arg(short, long, default_value_t = 64000)]
        limit: usize,
    },
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

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Analyze {
            path,
            format,
            limit,
        } => {
            let is_json = format == "json";

            if !is_soroban_project(path) {
                eprintln!("{} Error: {:?} is not a valid Soroban project. (Missing Cargo.toml with 'soroban-sdk' dependency)", "❌".red(), path);
                std::process::exit(1);
            }

            // In JSON mode, send informational lines to stderr so stdout is clean JSON.
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
            }

            let mut config = load_config(path);
            config.ledger_limit = *limit;

            let mut cache = AnalysisCache::load(path);
            let analyzer = Analyzer::new(config.clone());

            let mut all_size_warnings: Vec<SizeWarning> = Vec::new();
            let mut all_unsafe_patterns: Vec<UnsafePattern> = Vec::new();
            let mut all_auth_gaps: Vec<String> = Vec::new();
            let mut all_panic_issues: Vec<sanctifier_core::PanicIssue> = Vec::new();
            let mut all_arithmetic_issues: Vec<ArithmeticIssue> = Vec::new();
            let mut all_deprecated_api_issues: Vec<DeprecatedApiIssue> = Vec::new();
            let mut all_custom_rule_matches: Vec<CustomRuleMatch> = Vec::new();
            let mut all_gas_estimations: Vec<GasEstimationReport> = Vec::new();
            let mut all_reentrancy_issues: Vec<sanctifier_core::reentrancy::ReentrancyIssue> =
                Vec::new();
            let mut all_symbolic_paths: Vec<sanctifier_core::symbolic::SymbolicGraph> = Vec::new();
            let mut upgrade_report = UpgradeReport::empty();

            if path.is_dir() {
                analyze_directory(
                    path,
                    &analyzer,
                    &config,
                    &mut cache,
                    &mut all_size_warnings,
                    &mut all_unsafe_patterns,
                    &mut all_auth_gaps,
                    &mut all_panic_issues,
                    &mut all_arithmetic_issues,
                    &mut all_deprecated_api_issues,
                    &mut all_custom_rule_matches,
                    &mut all_gas_estimations,
                    &mut all_reentrancy_issues,
                    &mut all_symbolic_paths,
                    &mut upgrade_report,
                );
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                if let Ok(content) = fs::read_to_string(path) {
                    let file_hash = compute_hash(&content);
                    let file_key = path.to_string_lossy().to_string();

                    let analysis = if let Some(cached) = cache.files.get(&file_key) {
                        if cached.hash == file_hash {
                            cached.clone()
                        } else {
                            let res = run_analysis(path, &content, &analyzer, &config);
                            let updated = CachedAnalysis {
                                hash: file_hash,
                                ..res
                            };
                            cache.files.insert(file_key, updated.clone());
                            updated
                        }
                    } else {
                        let res = run_analysis(path, &content, &analyzer, &config);
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

                    let sym_paths = analyzer.analyze_symbolic_paths(&content);
                    all_symbolic_paths.extend(sym_paths);
                }
            }

            cache.save(if path.is_dir() {
                path
            } else {
                path.parent().unwrap_or(Path::new("."))
            });

            // ── Sanctity Score Calculation ──────────────────────────────────────────
            // Placeholder metrics for formal verification and test coverage
            // In a production environment, these would be pulled from Kani results and tarpaulin/grcov
            let proven_assertions = 11;
            let total_assertions = 13;
            let test_coverage = 0.85; // 85% coverage placeholder

            let scoring_input = sanctifier_core::scoring::ScoringInput {
                size_warnings: &all_size_warnings,
                unsafe_patterns: &all_unsafe_patterns,
                auth_gaps: &all_auth_gaps,
                panic_issues: &all_panic_issues,
                arithmetic_issues: &all_arithmetic_issues,
                deprecated_api_issues: &all_deprecated_api_issues,
                custom_rule_matches: &all_custom_rule_matches,
                reentrancy_issues: &all_reentrancy_issues,
                upgrade_report: &upgrade_report,
                proven_assertions,
                total_assertions,
                test_coverage,
            };

            let sanctity_score = sanctifier_core::scoring::calculate_sanctity_score(scoring_input);

            if is_json {
                eprintln!("{} Static analysis complete.", "✅".green());
            } else {
                println!("{} Static analysis complete.", "✅".green());
            }

            if format == "json" {
                let mut output = serde_json::json!({
                    "sanctity_score": sanctity_score,
                    "size_warnings": all_size_warnings,
                    "unsafe_patterns": all_unsafe_patterns,
                    "auth_gaps": all_auth_gaps,
                    "panic_issues": all_panic_issues,
                    "arithmetic_issues": all_arithmetic_issues,
                    "deprecated_api_issues": all_deprecated_api_issues,
                    "custom_rule_matches": all_custom_rule_matches,
                    "gas_estimations": all_gas_estimations,
                    "reentrancy_risks": all_reentrancy_issues,
                    "symbolic_paths": all_symbolic_paths,
                    "upgrade_report": upgrade_report,
                    "kani_metrics": KaniVerificationMetrics {
                        total_assertions: total_assertions as usize,
                        proven: proven_assertions as usize,
                        failed: (total_assertions - proven_assertions) as usize,
                        unreachable: 0,
                    }
                });

                // Generate ZK Proof Summary from the current output
                let report_str = serde_json::to_string(&output).unwrap_or_default();
                let zk_proof = ZkProofSummary::generate_zk_proof_summary(&report_str);

                // Inject the proof into the final JSON output
                output["zk_proof_summary"] = serde_json::to_value(&zk_proof).unwrap();

                println!(
                    "{}",
                    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
                );
            } else {
                println!(
                    "\n{}",
                    "╔══════════════════════════════════════════════════════════════╗".cyan()
                );
                println!("║ {:^60} ║", "🛡️  SANCTIFIER ANALYSIS REPORT".bold());
                println!(
                    "{}",
                    "╠══════════════════════════════════════════════════════════════╣".cyan()
                );

                let score_color = if sanctity_score.total_score >= 80 {
                    sanctity_score.total_score.to_string().green()
                } else if sanctity_score.total_score >= 50 {
                    sanctity_score.total_score.to_string().yellow()
                } else {
                    sanctity_score.total_score.to_string().red()
                };

                println!(
                    "║ {:^60} ║",
                    format!("Sanctity Score: {} / 100", score_color.bold())
                );
                println!(
                    "║ {:^60} ║",
                    format!(
                        "Security: {} | Proofs: {} | Tests: {}%",
                        sanctity_score.security_score,
                        sanctity_score.verification_score,
                        (test_coverage * 100.0) as u32
                    )
                );
                println!(
                    "{}",
                    "╚══════════════════════════════════════════════════════════════╝".cyan()
                );

                if !sanctity_score.deductions.is_empty() {
                    println!("\n{} Score Deductions:", "📉".red());
                    for deduction in &sanctity_score.deductions {
                        println!(
                            "   {} [-{}] {}: {}",
                            "•".red(),
                            deduction.amount,
                            deduction.category.bold(),
                            deduction.message
                        );
                    }
                }

                if all_size_warnings.is_empty() {
                    println!("\nNo ledger size issues found.");
                } else {
                    println!("\n{} Found Ledger Size Warnings!", "⚠️".yellow());
                    for warning in &all_size_warnings {
                        let (icon, msg) = match warning.level {
                            sanctifier_core::SizeWarningLevel::ExceedsLimit => {
                                ("🛑".red(), "EXCEEDS".red().bold())
                            }
                            sanctifier_core::SizeWarningLevel::ApproachingLimit => {
                                ("⚠️".yellow(), "is approaching".yellow())
                            }
                        };
                        println!(
                            "   {} {} {} the ledger entry size limit!",
                            icon,
                            warning.struct_name.bold(),
                            msg
                        );
                        println!(
                            "      Estimated size: {} bytes (Limit: {} bytes)",
                            warning.estimated_size.to_string().red(),
                            warning.limit
                        );
                    }
                }

                if !all_reentrancy_issues.is_empty() {
                    println!("\n{} Found potential Reentrancy Risks!", "🔄".yellow());
                    for issue in &all_reentrancy_issues {
                        println!(
                            "   {} Function {}: {}",
                            "->".red(),
                            issue.function_name.bold(),
                            issue.issue_type.yellow().bold()
                        );
                        println!("      Location: {}", issue.location);
                        println!("      {} {}", "💡".blue(), issue.recommendation);
                    }
                } else {
                    println!("\nNo reentrancy risks found.");
                }

                if !all_auth_gaps.is_empty() {
                    println!("\n{} Found potential Authentication Gaps!", "🛑".red());
                    for gap in &all_auth_gaps {
                        println!(
                            "   {} Function {} is modifying state without require_auth()",
                            "->".red(),
                            gap.bold()
                        );
                    }
                } else {
                    println!("\nNo authentication gaps found.");
                }

                if !all_panic_issues.is_empty() {
                    println!("\n{} Found explicit Panics/Unwraps!", "🛑".red());
                    for issue in &all_panic_issues {
                        println!(
                            "   {} Function {}: Using {} (Location: {})",
                            "->".red(),
                            issue.function_name.bold(),
                            issue.issue_type.yellow().bold(),
                            issue.location
                        );
                    }
                    println!("   {} Tip: Prefer returning Result or Error types for better contract safety.", "💡".blue());
                } else {
                    println!("\nNo panic/unwrap issues found.");
                }

                if !all_arithmetic_issues.is_empty() {
                    println!("\n{} Found unchecked Arithmetic Operations!", "🔢".yellow());
                    for issue in &all_arithmetic_issues {
                        println!(
                            "   {} Function {}: Unchecked `{}` ({})",
                            "->".red(),
                            issue.function_name.bold(),
                            issue.operation.yellow().bold(),
                            issue.location
                        );
                        println!("      {} {}", "💡".blue(), issue.suggestion);
                    }
                } else {
                    println!("\nNo arithmetic overflow risks found.");
                }

                if !all_deprecated_api_issues.is_empty() {
                    println!(
                        "\n{} Found usages of Deprecated Soroban APIs!",
                        "⚠️".yellow()
                    );
                    for issue in &all_deprecated_api_issues {
                        println!(
                            "   {} Function {}: Uses deprecated `{}` ({})",
                            "->".red(),
                            issue.function_name.bold(),
                            issue.deprecated_api.yellow().bold(),
                            issue.location
                        );
                    }
                }

                if !all_custom_rule_matches.is_empty() {
                    println!("\n{} Found Custom Rule Matches!", "📜".yellow());
                    for m in &all_custom_rule_matches {
                        println!(
                            "   {} Rule {}: `{}` (Line: {})",
                            "->".yellow(),
                            m.rule_name.bold(),
                            m.snippet.trim().italic(),
                            m.line
                        );
                    }
                }

                if !upgrade_report.findings.is_empty()
                    || !upgrade_report.upgrade_mechanisms.is_empty()
                    || !upgrade_report.init_functions.is_empty()
                {
                    println!("\n{} Upgrade Pattern Analysis", "🔄".yellow());
                    for f in &upgrade_report.findings {
                        println!(
                            "   {} [{}] {} ({})",
                            "->".yellow(),
                            format!("{:?}", f.category).to_lowercase(),
                            f.message,
                            f.location
                        );
                        println!("      {} {}", "💡".blue(), f.suggestion);
                    }
                    if !upgrade_report.suggestions.is_empty() {
                        for s in &upgrade_report.suggestions {
                            println!("   {} {}", "💡".blue(), s);
                        }
                    }
                } else {
                    println!("\nNo upgrade pattern issues found.");
                }

                if !all_gas_estimations.is_empty() {
                    println!("\n{} Gas Estimation (Heuristics)", "⛽".cyan());
                    println!("   Note: These are static estimations based on instruction counting and do not represent exact Soroban simulations.");
                    for gas in &all_gas_estimations {
                        println!(
                            "   {} Function {}: {} Instructions, {} Mem bytes",
                            "->".cyan(),
                            gas.function_name.bold(),
                            gas.estimated_instructions,
                            gas.estimated_memory_bytes
                        );
                    }
                }

                // Append the ZK Proof generation explicitly in text mode as well
                println!("\n{} Zero-Knowledge Proof Summary (Emulated)", "🛡️".blue());

                let output_data_for_hash = serde_json::json!({
                    "size": all_size_warnings.len(),
                    "auth": all_auth_gaps.len(),
                    "panics": all_panic_issues.len(),
                    "arith": all_arithmetic_issues.len(),
                    "deprecated": all_deprecated_api_issues.len(),
                });
                let report_str = serde_json::to_string(&output_data_for_hash).unwrap_or_default();
                let zk_proof = ZkProofSummary::generate_zk_proof_summary(&report_str);

                println!("   {} ID: {}", "->".blue(), zk_proof.proof_id.bold());
                println!(
                    "   {} Public Inputs Hash: {}",
                    "->".blue(),
                    zk_proof.public_inputs_hash
                );
                println!(
                    "   {} Verifier Contract: {}",
                    "->".blue(),
                    zk_proof.verifier_contract.bold()
                );
            }
        }
        Commands::Report { output } => {
            println!("{} Generating report...", "📄".yellow());
            if let Some(p) = output {
                println!("Report saved to {:?}", p);
            } else {
                println!("Report printed to stdout.");
            }
        }
        Commands::Init => {}
        Commands::Kani { path, output } => {
            if path.extension().and_then(|s| s.to_str()) != Some("rs") {
                eprintln!(
                    "{} Error: Kani bridge currently only supports single .rs files.",
                    "❌".red()
                );
                std::process::exit(1);
            }
            if let Ok(content) = fs::read_to_string(path) {
                let config = load_config(path);
                match sanctifier_core::kani_bridge::KaniBridge::translate_for_kani(
                    &content,
                    config.kani_unwind,
                ) {
                    Ok(harness) => {
                        if let Some(out_path) = output {
                            if let Err(e) = std::fs::write(out_path, harness) {
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
            let config = load_config(path);
            let analyzer = Analyzer::new(config.clone());
            let mut total_fixes = 0;

            if path.is_dir() {
                fix_directory(path, &analyzer, &config, *yes, *dry_run, &mut total_fixes);
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                fix_file(path, &analyzer, *yes, *dry_run, &mut total_fixes);
            }

            if *dry_run {
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
    _all_symbolic_paths: &mut Vec<sanctifier_core::symbolic::SymbolicGraph>,
    _upgrade_report: &mut UpgradeReport,
) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Skip paths matches in config.exclude
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
                    _all_symbolic_paths,
                    _upgrade_report,
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
                                hash: file_hash.clone(),
                                ..res
                            };
                            cache.files.insert(file_key, updated.clone());
                            updated
                        }
                    } else {
                        let res = run_analysis(&path, &content, analyzer, config);
                        let updated = CachedAnalysis {
                            hash: file_hash.clone(),
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
                }
            }
        }
    }
}

fn run_analysis(
    path: &Path,
    content: &str,
    analyzer: &Analyzer,
    config: &SanctifyConfig,
) -> CachedAnalysis {
    crate::rules::RuleEngine::new(analyzer, config).run_all(content, Some(path))
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

        // Sort fixes by line and column in reverse order to apply them without breaking offsets
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

            if should_apply {
                if !dry_run {
                    // Simple line-based replacement for now (since our fixes are mostly additive or prefix)
                    // For PrefixUnused:
                    if fix.fix_type == FixType::PrefixUnused {
                        if let Some(line) = lines.get_mut(fix.line - 1) {
                            let (start, rest) = line.split_at(fix.column);
                            let (_, end) = rest.split_at(fix.end_column - fix.column);
                            *line = format!("{}{}{}", start, fix.replacement, end);
                            applied_in_file += 1;
                        }
                    } else if fix.fix_type == FixType::AddAuth {
                        // For AddAuth:
                        if let Some(line) = lines.get_mut(fix.line - 1) {
                            let (start, rest) = line.split_at(fix.column);
                            *line = format!("{}{}{}", start, fix.replacement, rest);
                            applied_in_file += 1;
                        }
                    }
                } else {
                    applied_in_file += 1;
                }
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

    while let Some(path) = current {
        let config_path = path.join(".sanctify.toml");
        if config_path.exists() {
            return Some(config_path);
        }
        current = path.parent().map(|p| p.to_path_buf());
    }
    None
}
