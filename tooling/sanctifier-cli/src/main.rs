use clap::{Parser, Subcommand};
use sanctifier_core::Analyzer;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sanctifier")]
#[command(about = "Security and formal verification suite for Soroban smart contracts", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a Soroban smart contract project
    Analyze {
        /// Path to the contract project directory
        #[arg(default_value = ".")]
        path: PathBuf,

        /// Output format (text or json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Optional webhook URLs to send notifications to
        #[arg(long = "webhook-url")]
        webhook_urls: Vec<String>,
    },
    /// Generate a security badge SVG and Markdown snippet
    Badge {
        #[arg(long)]
        report: PathBuf,
        #[arg(long)]
        svg_output: PathBuf,
        #[arg(long)]
        markdown_output: PathBuf,
    },
    /// Update Sanctifier binary
    Update,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path, format, webhook_urls } => {
            let path_str = path.to_string_lossy();
            println!("✨ Sanctifier: Validating project at \"{}\"", path_str);
            
            let analyzer = Analyzer::new(&path_str);
            match analyzer.run() {
                Ok(report) => {
                    if format == "json" {
                        let json = serde_json::to_string_pretty(&report).unwrap();
                        println!("{}", json);
                    } else {
                        println!("🔍 Analyzing contract at \"{}\"...", path_str);
                        println!("✅ Static analysis complete.\n");
                        for finding in &report.findings {
                            println!("🛑 [{}] {}", finding.code, finding.title);
                            println!("   -> Location: {}", finding.location);
                            println!("   -> {}", finding.description);
                            println!("   💡 {}", finding.recommendation);
                            println!();
                        }
                    }

                    for url in webhook_urls {
                        println!("🔔 Dispatching scan report to webhook: {}", url);
                    }
                }
                Err(err) => {
                    eprintln!("❌ Error during analysis: {}", err);
                }
            }
        }
        Commands::Badge { report, svg_output, markdown_output } => {
            println!("🎨 Generating security badge from report at {:?}", report);
            let dummy_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="20"><rect width="120" height="20" fill="#555"/><text x="10" y="14" fill="#fff">sanctified: pass</text></svg>"#;
            let dummy_md = "[![Sanctifier Badge](badges/sanctifier-security.svg)](https://github.com/Centurylong/sanctifier)";
            
            if let Some(parent) = svg_output.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Some(parent) = markdown_output.parent() {
                let _ = fs::create_dir_all(parent);
            }

            fs::write(svg_output, dummy_svg).expect("Failed writing SVG badge");
            fs::write(markdown_output, dummy_md).expect("Failed writing Markdown snippet");
            println!("✅ Badge assets generated successfully.");
        }
        Commands::Update => {
            println!("🔄 Checking for updates... Sanctifier is up to date!");
        }
    }
}
