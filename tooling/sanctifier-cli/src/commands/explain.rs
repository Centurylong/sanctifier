//! `sanctifier explain <CODE>` - prints the finding-code catalog entry

use clap::Args;
use sanctifier_core::finding_codes;

#[derive(Args, Debug)]
pub struct ExplainArgs {
    /// Finding code to explain (e.g., S001, S002)
    pub code: String,
}

pub fn exec(args: ExplainArgs) -> anyhow::Result<()> {
    let code_upper = args.code.to_uppercase();
    let all_codes = finding_codes::all_finding_codes();

    match all_codes.iter().find(|fc| fc.code == code_upper) {
        Some(fc) => {
            println!("Finding Code: {}", fc.code);
            println!("Category:     {}", fc.category);
            println!("Description:  {}", fc.description);
            println!();
            println!("Reference: https://github.com/Codex723/sanctifier/blob/main/docs/error-codes.md");
        }
        None => {
            eprintln!("Unknown finding code: '{}'", args.code);
            eprintln!();
            eprintln!("Available codes:");
            for fc in &all_codes {
                eprintln!("  {} - {}", fc.code, fc.category);
            }
            std::process::exit(1);
        }
    }

    Ok(())
}