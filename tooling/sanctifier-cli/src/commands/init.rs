use clap::Args;
use colored::Colorize;
use sanctifier_core::{CustomRule, SanctifyConfig};
use std::fs;
use std::path::{Path, PathBuf};

/// Contract template to scaffold with `sanctifier init --template <TEMPLATE>`.
#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum Template {
    /// SEP-41 fungible token with auth, overflow, and supply-cap checks baked in
    Token,
    /// Constant-product AMM with slippage guard and K-invariant enforcement
    Amm,
    /// M-of-N multi-signature governance with timelock and nonce replay protection
    Multisig,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Force overwrite existing configuration file
    #[arg(short, long)]
    pub force: bool,

    /// Contract template to scaffold (token | amm | multisig)
    #[arg(long, value_enum)]
    pub template: Option<Template>,

    /// Output directory for the scaffolded contract [default: current dir]
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

pub struct ConfigGenerator;

impl ConfigGenerator {
    pub fn generate_default_config() -> SanctifyConfig {
        SanctifyConfig {
            ignore_paths: vec!["target".to_string(), ".git".to_string()],
            enabled_rules: vec![
                "auth_gaps".to_string(),
                "panics".to_string(),
                "arithmetic".to_string(),
                "ledger_size".to_string(),
            ],
            ledger_limit: 64000,
            strict_mode: false,
            custom_rules: vec![
                CustomRule {
                    name: "no_unsafe_block".to_string(),
                    pattern: "unsafe\\s*\\{".to_string(),
                    severity: sanctifier_core::RuleSeverity::Error,
                },
                CustomRule {
                    name: "no_mem_forget".to_string(),
                    pattern: "std::mem::forget".to_string(),
                    severity: sanctifier_core::RuleSeverity::Warning,
                },
            ],
            approaching_threshold: 0.8,
        }
    }
}

pub struct FileWriter;

impl FileWriter {
    pub fn config_exists(path: &Path) -> bool {
        path.join(".sanctify.toml").exists()
    }

    pub fn write_config(config: &SanctifyConfig, path: &Path) -> anyhow::Result<PathBuf> {
        let config_path = path.join(".sanctify.toml");
        let toml_string = toml::to_string_pretty(config)?;
        fs::write(&config_path, toml_string)?;
        Ok(config_path)
    }
}

pub struct OutputFormatter;

impl OutputFormatter {
    pub fn display_success(config_path: &Path) {
        println!("{} Configuration file created successfully!", "\u2713".green());
        println!("   Location: {}", config_path.display());
    }

    pub fn display_existing_file_warning() {
        eprintln!(
            "{} Configuration file already exists: .sanctify.toml",
            "\u26a0".yellow()
        );
        eprintln!("   Use --force to overwrite the existing configuration");
    }

    pub fn display_error(error: &anyhow::Error) {
        eprintln!("{} Failed to create configuration file", "\u2717".red());
        eprintln!("   Error: {}", error);
    }

    pub fn display_template_hint() {
        println!();
        println!("   {} Scaffold a secure contract template with --template:", "\u2192".cyan());
        println!("     sanctifier init --template token     # SEP-41 fungible token");
        println!("     sanctifier init --template amm       # Constant-product AMM");
        println!("     sanctifier init --template multisig  # M-of-N governance");
    }
}
    pub fn display_scaffold_success(template_name: &str, output_dir: &Path) {
        println!("{} Scaffolded {} contract successfully!", "\u2713".green(), template_name.bold());
        println!("   Location: {}", output_dir.display());
    }

    pub fn display_scaffold_files(files: &[&Path]) {
        println!("   Files created:");
        for f in files {
            println!("     {} {}", "\u2022".green(), f.display());
        }
    }
}
