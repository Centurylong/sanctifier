use crate::commands::analyze::{self, AnalyzeArgs};
use clap::Args;
use sanctifier_core::FailOnLevel;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct CiArgs {
    /// Path to the contract directory or Cargo.toml
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format (e.g. text, json, sarif)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Exit with non-zero status when findings at or above this severity exist (critical, high, medium, low, none)
    #[arg(long, value_name = "LEVEL")]
    pub fail_on: Option<FailOnLevel>,
}

pub fn exec(args: CiArgs) -> anyhow::Result<()> {
    println!("sanctifier ci — running full analysis and verification gate");
    println!();

    let analyze_args = AnalyzeArgs {
        path: args.path,
        format: args.format,
        limit: 64000,
        vuln_db: None,
        webhook_urls: vec![],
        no_baseline: false,
        max_memory: Some(1024),
        profile: false,
        fail_on: Some(args.fail_on.unwrap_or(FailOnLevel::High)),
    };

    analyze::exec(analyze_args)
}

