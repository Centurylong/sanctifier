use clap::{Parser, Subcommand};
use colored::*;
use sanctifier_core::{callgraph_to_dot, Analyzer, SanctifyConfig};
use std::fs;
use std::path::{Path, PathBuf};

mod branding;
mod commands;
mod score;
pub mod vulndb;
pub mod zk;

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
    /// Generate (or verify) a zero-knowledge attestation that a scan passed a score threshold
    Attest(commands::attest::AttestArgs),
    /// Generate a dynamic Sanctifier status badge
    Badge(commands::badge::BadgeArgs),
    /// Generate a security report
    Report {
        /// Output file path
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },
    /// Initialize Sanctifier in a new project
    Init(commands::init::InitArgs),
    /// Generate a Graphviz DOT call graph of cross-contract calls (env.invoke_contract)
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
    /// Verify #[sanctify::invariant] declarations across a contract or workspace
    Verify(commands::verify::VerifyArgs),
    /// Run SMT-based formal verification on Soroban token contract invariants
    Prove(commands::prove::ProveArgs),
    /// (internal) Regenerate the Markdown CLI reference from the clap definitions.
    ///
    /// Prints the reference to stdout. Hidden from `--help`; used by the docs
    /// staleness check in CI to guarantee `docs/cli.md` never drifts from the
    /// parser. Regenerate locally with:
    ///   `cargo run -q -p sanctifier-cli -- generate-docs > docs/cli.md`
    #[command(hide = true)]
    GenerateDocs,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze(args) => {
            if args.format != "json" {
                branding::print_logo();
            }
            commands::analyze::exec(args)?;
        }
        Commands::Attest(args) => {
            commands::attest::exec(args)?;
        }
        Commands::Badge(args) => {
            commands::badge::exec(args)?;
        }
        Commands::Report { output } => {
            if let Some(p) = output {
                println!("Report saved to {:?}", p);
            } else {
                println!("Report printed to stdout.");
            }
        }
        Commands::Init(args) => {
            commands::init::exec(args, None)?;
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
                eprintln!("{} Failed to write DOT file: {}", "❌".red(), e);
                std::process::exit(1);
            }
            println!(
                "{} Wrote call graph to {:?} ({} edges)",
                "✅".green(),
                output,
                edges.len()
            );
        }
        Commands::Update => {
            commands::update::exec()?;
        }
        Commands::Verify(args) => {
            commands::verify::exec(args)?;
        }
        Commands::Prove(args) => {
            commands::prove::exec(args)?;
        }
        Commands::GenerateDocs => {
            // Render the full command tree to Markdown straight from the clap
            // definitions so the committed `docs/cli.md` can never describe a
            // flag the parser doesn't have. The CI staleness check regenerates
            // this and fails on any diff.
            print!(
                "<!-- \
                 DO NOT EDIT THIS FILE BY HAND. It is generated from the clap \
                 command definitions in tooling/sanctifier-cli/src by \
                 `cargo run -p sanctifier-cli -- generate-docs > docs/cli.md` and \
                 verified in CI. Edit the `#[command]`/`#[arg]` doc comments instead. \
                 -->\n\n"
            );
            print!("{}", clap_markdown::help_markdown::<Cli>());
        }
    }

    Ok(())
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
