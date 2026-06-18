use clap::Args;
use colored::Colorize;
use sanctifier_core::{
    invariant::{InvariantDecl, InvariantVerifyResult, SmtInvariantVerifier},
    Analyzer, SanctifyConfig,
};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct VerifyArgs {
    /// Path to a contract directory, workspace directory, or a single .rs file.
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Exit with a non-zero status code if any invariant cannot be proven
    /// (Refuted or Unknown). Useful in CI.
    #[arg(long, default_value_t = false)]
    pub strict: bool,

    /// Emit results as JSON instead of human-readable text.
    #[arg(long, default_value_t = false)]
    pub json: bool,
}

/// Recursively collect every `.rs` file under `dir`, skipping paths that
/// contain any segment in `ignore` (e.g. "target", ".git").
pub(crate) fn collect_rs_files(dir: &Path, ignore: &[String], out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if ignore.iter().any(|p| name.contains(p.as_str())) {
                continue;
            }
            collect_rs_files(&path, ignore, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Scan `path` (file or directory) and return all invariant declarations found.
pub(crate) fn discover_invariants(path: &Path) -> Vec<InvariantDecl> {
    let config = SanctifyConfig::default();
    let analyzer = Analyzer::new(config.clone());

    let mut rs_files: Vec<PathBuf> = Vec::new();
    if path.is_dir() {
        collect_rs_files(path, &config.ignore_paths, &mut rs_files);
    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
        rs_files.push(path.to_path_buf());
    }

    let mut all_decls: Vec<InvariantDecl> = Vec::new();
    for file in rs_files {
        let source = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let label = file.display().to_string();
        let decls = analyzer.scan_invariant_attrs(&source, &label);
        all_decls.extend(decls);
    }
    all_decls
}

/// Run the SMT verifier over all discovered invariants and return paired results.
///
/// Returns `(InvariantDecl, InvariantVerifyResult)` for every invariant found.
/// When the `smt` feature is absent the function returns `Unsupported` for
/// everything so the CLI can still print a meaningful message.
pub(crate) fn run_verification(
    decls: Vec<InvariantDecl>,
) -> Vec<(InvariantDecl, InvariantVerifyResult)> {
    if decls.is_empty() {
        return vec![];
    }

    let verifier = SmtInvariantVerifier::new();
    verifier.verify_all(&decls)
}

pub fn exec(args: VerifyArgs) -> anyhow::Result<()> {
    let decls = discover_invariants(&args.path);

    if decls.is_empty() {
        if !args.json {
            println!(
                "{} No #[sanctify::invariant] attributes found in {:?}",
                "ℹ".cyan(),
                args.path
            );
        } else {
            println!("[]");
        }
        return Ok(());
    }

    let results = run_verification(decls);

    if args.json {
        let json = serde_json::to_string_pretty(&results.iter().map(|(d, r)| {
            serde_json::json!({
                "contract": d.contract_name,
                "invariant": d.expr_str,
                "location": d.location,
                "result": r,
            })
        }).collect::<Vec<_>>())?;
        println!("{}", json);
    } else {
        println!(
            "\n{}\n",
            "─── sanctifier verify ───────────────────────────────".bold()
        );
        for (decl, result) in &results {
            let status = match result {
                InvariantVerifyResult::Proven => "  PROVEN  ".on_green().black().bold(),
                InvariantVerifyResult::Refuted { .. } => "  REFUTED ".on_red().white().bold(),
                InvariantVerifyResult::Unknown => " UNKNOWN  ".on_yellow().black().bold(),
                InvariantVerifyResult::Unsupported => "  KANI ↗  ".on_blue().white().bold(),
            };
            println!(
                "{} {} :: {}",
                status,
                decl.contract_name.bold(),
                decl.expr_str.cyan()
            );
            println!("         {}", decl.location.dimmed());
            if let InvariantVerifyResult::Refuted { counterexample } = result {
                println!(
                    "         {} {}",
                    "counterexample:".red().bold(),
                    counterexample
                );
            }
            if *result == InvariantVerifyResult::Unsupported {
                println!(
                    "         {} run: {}",
                    "→".blue(),
                    "cargo kani --target-dir /tmp/kani".dimmed()
                );
            }
            println!();
        }

        let proven = results.iter().filter(|(_, r)| *r == InvariantVerifyResult::Proven).count();
        let refuted = results.iter().filter(|(_, r)| matches!(r, InvariantVerifyResult::Refuted { .. })).count();
        let kani = results.iter().filter(|(_, r)| *r == InvariantVerifyResult::Unsupported).count();
        println!(
            "{} {} proven  {} refuted  {} dispatched to Kani",
            "Summary:".bold(),
            proven.to_string().green().bold(),
            refuted.to_string().red().bold(),
            kani.to_string().blue().bold(),
        );
    }

    if args.strict {
        let has_failure = results.iter().any(|(_, r)| {
            matches!(r, InvariantVerifyResult::Refuted { .. } | InvariantVerifyResult::Unknown)
        });
        if has_failure {
            std::process::exit(1);
        }
    }

    Ok(())
}
