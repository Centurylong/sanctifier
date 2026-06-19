use clap::Args;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unified configuration with layered precedence:
/// CLI flags > env (SANCTIFY_*) > .sanctify.toml > defaults
#[derive(Debug, Clone, Serialize, Deserialize, Args)]
pub struct CliConfig {
    /// Path to the contract directory or Cargo.toml
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format (table, json, sarif, junit)
    #[arg(short, long, default_value = "table", env = "SANCTIFY_FORMAT")]
    pub format: String,

    /// Limit for ledger entry size in bytes
    #[arg(short = 'L', long, default_value = "64000", env = "SANCTIFY_LEDGER_LIMIT")]
    pub limit: usize,

    /// Path to a custom vulnerability database JSON file
    #[arg(long, env = "SANCTIFY_VULN_DB")]
    pub vuln_db: Option<PathBuf>,

    /// Webhook endpoint(s) to notify when scan completes
    #[arg(long = "webhook-url", env = "SANCTIFY_WEBHOOK_URL")]
    pub webhook_urls: Vec<String>,

    /// Minimum severity to fail on (critical, high, medium, low)
    #[arg(long = "fail-on", default_value = "critical", env = "SANCTIFY_FAIL_ON")]
    pub fail_on: String,

    /// Path to an explicit config file
    #[arg(long, env = "SANCTIFY_CONFIG")]
    pub config: Option<PathBuf>,

    /// Enable verbose logging (-v, -vv)
    #[arg(short, long, action = clap::ArgAction::Count, env = "SANCTIFY_VERBOSE")]
    pub verbose: u8,

    /// Quiet mode (suppress progress output)
    #[arg(short, long, env = "SANCTIFY_QUIET")]
    pub quiet: bool,

    /// Disable colored output
    #[arg(long = "no-color", env = "SANCTIFY_NO_COLOR")]
    pub no_color: bool,

    /// CI platform for annotation output (github, gitlab, none)
    #[arg(long, default_value = "none", env = "SANCTIFY_CI")]
    pub ci: String,
}

impl CliConfig {
    /// Build config with figment-based layered precedence:
    /// CLI flags > env (SANCTIFY_*) > .sanctify.toml > defaults
    pub fn from_figment(cli_args: &CliConfig) -> Self {
        // Start with defaults
        let mut cfg = CliConfig::default();

        // Layer: .sanctify.toml config file (discovered via walk-up or explicit)
        let config_path = cli_args
            .config
            .clone()
            .or_else(|| Self::find_config_upwards(&cli_args.path));

        if let Some(ref cp) = config_path {
            let figment = Figment::from(Toml::file(cp));
            if let Ok(file_cfg) = figment.extract::<CliConfig>() {
                cfg.merge(file_cfg);
            }
        }

        // Layer: env vars prefixed with SANCTIFY_
        let figment = Figment::from(Env::prefixed("SANCTIFY_"));
        if let Ok(env_cfg) = figment.extract::<CliConfig>() {
            cfg.merge(env_cfg);
        }

        // Layer: CLI args (highest priority)
        cfg.merge(cli_args.clone());

        cfg
    }

    /// Walk up from start_path looking for .sanctify.toml
    fn find_config_upwards(start_path: &std::path::Path) -> Option<PathBuf> {
        let mut current = if start_path.is_file() {
            start_path.parent()?.to_path_buf()
        } else {
            start_path.to_path_buf()
        };
        loop {
            let candidate = current.join(".sanctify.toml");
            if candidate.exists() {
                return Some(candidate);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    /// Merge another config into this one (non-None/non-default values win)
    fn merge(&mut self, other: CliConfig) {
        if other.format != "table" {
            self.format = other.format;
        }
        if other.limit != 64000 {
            self.limit = other.limit;
        }
        if other.vuln_db.is_some() {
            self.vuln_db = other.vuln_db;
        }
        if !other.webhook_urls.is_empty() {
            self.webhook_urls = other.webhook_urls;
        }
        if other.fail_on != "critical" {
            self.fail_on = other.fail_on;
        }
        if other.config.is_some() {
            self.config = other.config;
        }
        if other.verbose > 0 {
            self.verbose = other.verbose;
        }
        if other.quiet {
            self.quiet = other.quiet;
        }
        if other.no_color {
            self.no_color = other.no_color;
        }
        if other.ci != "none" {
            self.ci = other.ci;
        }
        // Always use the path from the highest priority source
        self.path = other.path;
    }

    /// Determine if output should use colors
    pub fn use_color(&self) -> bool {
        !self.no_color && atty::is(atty::Stream::Stdout)
    }

    /// Determine if progress bars should be shown
    pub fn show_progress(&self) -> bool {
        !self.quiet && atty::is(atty::Stream::Stdout)
    }
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("."),
            format: "table".to_string(),
            limit: 64000,
            vuln_db: None,
            webhook_urls: vec![],
            fail_on: "critical".to_string(),
            config: None,
            verbose: 0,
            quiet: false,
            no_color: false,
            ci: "none".to_string(),
        }
    }
}