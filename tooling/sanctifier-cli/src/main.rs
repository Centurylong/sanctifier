use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use sanctifier_core::{callgraph_to_dot, Analyzer, SanctifyConfig};
use std::fs;
use std::path::{Path, PathBuf};

mod branding;
mod commands;
mod config;
mod logging;
mod reporter;
pub mod vulndb;

use tracing::{error, info};

#[derive(Parser)]
#[command(name = "sanctifier")]
#[command(about = "Stellar Soroban Security & Formal Verification Suite")]
#[command(after_help = "EXIT CODES:
  0 - No findings (or all findings are below the --fail-on threshold)
  1 - Findings found at or above the --fail-on severity
  2 - Internal error (invalid config, parse failure, etc.)")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Analyze a Soroban contract for vulnerabilities
    Analyze(commands::analyze::AnalyzeArgs),
    /// Generate a dynamic Sanctifier status badge
    Badge(commands::badge::BadgeArgs),
    /// Generate a security report
    Report {
        /// Output file path
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,

        /// Output format (table, json, sarif, junit)
        #[arg(short, long, default_value = "json")]
        format: String,
    },
    /// Initialize Sanctifier in a new project
    Init(commands::init::InitArgs),
    /// Generate a Graphviz DOT call graph of cross-contract calls
    Callgraph {
        /// Path to a contract directory, workspace directory, or a single .rs file
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output DOT file path
        #[arg(short, long, default_value = "callgraph.dot")]
        output: PathBuf,
    },
    /// Check for and download the latest Sanctifier binary
    Update,
    /// Explain a finding code (e.g., S001, S002)
    Explain(commands::explain::ExplainArgs),
    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell)
        shell: Shell,
    },
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging early, before any subcommand runs
    logging::init_logging(0, false);

    let exit_code = match run(cli) {
        Ok(code) => code,
        Err(e) => {
            error!("{}", e);
            eprintln!("Error: {}", e);
            2
        }
    };

    std::process::exit(exit_code);
}

fn run(cli: Cli) -> anyhow::Result<i32> {
    match cli.command {
        Commands::Analyze(args) => {
            if args.format != "json" {
                branding::print_logo();
            }
            commands::analyze::exec(args)?;
            Ok(0)
        }
        Commands::Badge(args) => {
            commands::badge::exec(args)?;
            Ok(0)
        }
        Commands::Report { output, format: _ } => {
            if let Some(p) = output {
                info!("Report saved to {:?}", p);
            } else {
                println!("Report printed to stdout.");
            }
            Ok(0)
        }
        Commands::Init(args) => {
            commands::init::exec(args, None)?;
            Ok(0)
        }
        Commands::Callgraph { path, output } => {
            let config = load_config(&path);
            let analyzer = Analyzer::new(config.clone());

            let mut rs_files: Vec<PathBuf> = Vec::new();
            if path.is_dir() {
                collect_rs_files(&path, &config, &mut rs_files);
            } else {
                rs_files.push(path.clone());
            }

            let mut edges = Vec::new();
            for f in rs_files {
                if f.extension().and_then(|s| s.to_str()) != Some("rs") {
                    continue;
                }

                let content = match fs::read_to_string(&f) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let caller = infer_contract_name(&content).unwrap_or_else(|| {
                    f.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("<unknown>")
                        .to_string()
                });

                let file_label = f.display().to_string();
                edges.extend(analyzer.scan_invoke_contract_calls(&content, &caller, &file_label));
            }

            let dot = callgraph_to_dot(&edges);
            if let Err(e) = fs::write(&output, dot) {
                error!("Failed to write DOT file: {}", e);
                eprintln!("{} Failed to write DOT file: {}", "❌", e);
                return Ok(2);
            }
            info!("Wrote call graph to {:?} ({} edges)", output, edges.len());
            println!(
                "{} Wrote call graph to {:?} ({} edges)",
                "✅", output, edges.len()
            );
            Ok(0)
        }
        Commands::Update => {
            commands::update::exec()?;
            Ok(0)
        }
        Commands::Explain(args) => {
            commands::explain::exec(args)?;
            Ok(0)
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let mut stdout = std::io::stdout();
            clap_complete::generate(shell, &mut cmd, "sanctifier", &mut stdout);
            Ok(0)
        }
    }
}

fn collect_rs_files(dir: &Path, config: &SanctifyConfig, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if config.ignore_paths.iter().any(|p| name.contains(p)) {
                continue;
            }
            collect_rs_files(&path, config, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn infer_contract_name(source: &str) -> Option<String> {
    let mut saw_contract_attr = false;
    for line in source.lines() {
        let l = line.trim();
        if l.starts_with("#[contract]") {
            saw_contract_attr = true;
            continue;
        }
        if saw_contract_attr {
            if let Some(rest) = l.strip_prefix("pub struct ") {
                return Some(
                    rest.trim_end_matches(';')
                        .split_whitespace()
                        .next()?
                        .to_string(),
                );
            }
            if let Some(rest) = l.strip_prefix("struct ") {
                return Some(
                    rest.trim_end_matches(';')
                        .split_whitespace()
                        .next()?
                        .to_string(),
                );
            }
        }
    }
    None
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