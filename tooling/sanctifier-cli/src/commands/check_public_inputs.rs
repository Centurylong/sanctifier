//! `sanctifier check-public-inputs` (issue #740).
//!
//! Cross-checks a ZK circuit's declared public inputs against a Soroban
//! verifier contract's assumed public-input encoding. See
//! `docs/public-input-consistency.md` for the feasibility write-up and scope.

use anyhow::Context as _;
use clap::Args;
use colored::*;
use sanctifier_core::public_input_consistency::check_consistency;
use std::fs;
use std::path::PathBuf;

#[derive(Args)]
pub struct CheckPublicInputsArgs {
    /// Path to the circuit source file (e.g. tooling/zk/src/circuit.rs)
    #[arg(long)]
    pub circuit: PathBuf,

    /// Path to the verifier contract source file (e.g. contracts/zk-verifier/src/lib.rs)
    #[arg(long)]
    pub contract: PathBuf,

    /// Emit the result as JSON
    #[arg(long)]
    pub json: bool,
}

pub fn exec(args: CheckPublicInputsArgs) -> anyhow::Result<()> {
    let circuit_source = fs::read_to_string(&args.circuit)
        .with_context(|| format!("reading circuit file {:?}", args.circuit))?;
    let contract_source = fs::read_to_string(&args.contract)
        .with_context(|| format!("reading contract file {:?}", args.contract))?;

    let report = check_consistency(&circuit_source, &contract_source);

    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if report.mismatch {
        println!("{} {}", "❌".red(), report.message());
    } else {
        println!("{} {}", "✅".green(), report.message());
    }

    if report.mismatch {
        std::process::exit(1);
    }

    Ok(())
}
