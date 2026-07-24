use clap::Args;
use colored::*;
use sanctifier_core::wasm::{analyze_wasm, WasmReport, WasmSeverity};
use std::fs;
use std::path::PathBuf;

/// `sanctifier wasm` — source-optional analysis of a compiled Soroban module.
///
/// Point it at a `.wasm` artifact when the Rust source is not available. It runs
/// basic, bytecode-level checks and is explicit about what it cannot see
/// compared to source mode (`sanctifier analyze`).
#[derive(Args, Debug)]
pub struct WasmArgs {
    /// Path to a compiled `.wasm` module.
    pub path: PathBuf,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Print the source-vs-WASM limitations note (also shown at the end of text output).
    #[arg(long)]
    pub show_limitations: bool,
}

pub fn exec(args: WasmArgs) -> anyhow::Result<()> {
    let is_json = args.format == "json";

    let bytes = match fs::read(&args.path) {
        Ok(b) => b,
        Err(e) => {
            if is_json {
                let err = serde_json::json!({
                    "error": format!("could not read {:?}: {}", args.path, e),
                    "success": false,
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                eprintln!("{} Could not read {:?}: {}", "❌".red(), args.path, e);
            }
            std::process::exit(1);
        }
    };

    let report = match analyze_wasm(&bytes) {
        Ok(r) => r,
        Err(e) => {
            if is_json {
                let err = serde_json::json!({
                    "error": e.to_string(),
                    "success": false,
                });
                println!("{}", serde_json::to_string_pretty(&err)?);
            } else {
                eprintln!("{} {}", "❌".red(), e);
            }
            std::process::exit(1);
        }
    };

    if is_json {
        emit_json(&args, &report)?;
    } else {
        emit_text(&args, &report);
    }

    // Exit non-zero if any error-severity finding is present, so the command can
    // gate CI the same way `analyze` does.
    if report
        .findings
        .iter()
        .any(|f| f.severity == WasmSeverity::Error)
    {
        std::process::exit(1);
    }
    Ok(())
}

fn emit_json(args: &WasmArgs, report: &WasmReport) -> anyhow::Result<()> {
    let out = serde_json::json!({
        "path": args.path.display().to_string(),
        "mode": "source-optional-wasm",
        "is_soroban_contract": report.is_soroban_contract(),
        "module": report.info,
        "findings": report.findings,
        "limitations": WasmReport::limitations(),
        "summary": {
            "total_findings": report.findings.len(),
        },
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn emit_text(args: &WasmArgs, report: &WasmReport) {
    let info = &report.info;

    println!(
        "{} Source-optional WASM analysis of {:?}",
        "🧩".blue(),
        args.path
    );
    if report.is_soroban_contract() {
        println!("{} Detected a Soroban contract module.", "✨".green());
    } else {
        println!(
            "{} No Soroban contract spec found — analyzing as a generic WASM module.",
            "⚠️".yellow()
        );
    }

    println!("\n{}", "Module summary".bold());
    println!("   Functions defined : {}", info.num_functions);
    println!(
        "   Imports           : {} ({} functions){}",
        info.num_imports_total,
        info.num_func_imports,
        if info.import_modules.is_empty() {
            String::new()
        } else {
            format!(" from [{}]", info.import_modules.join(", "))
        }
    );
    println!("   Exported functions: {}", info.num_func_exports);
    if !info.export_names.is_empty() {
        let mut names = info.export_names.clone();
        names.sort();
        println!("      {}", names.join(", "));
    }
    match info.memory_pages_min {
        Some(min) => {
            let max = info
                .memory_pages_max
                .map(|m| m.to_string())
                .unwrap_or_else(|| "unbounded".to_string());
            println!("   Memory (64KiB pgs): min {min}, max {max}");
        }
        None => println!("   Memory (64KiB pgs): none declared"),
    }
    if !info.custom_sections.is_empty() {
        println!("   Custom sections   : {}", info.custom_sections.join(", "));
    }
    if info.has_start {
        println!("   Start function    : present");
    }

    if report.findings.is_empty() {
        println!("\n{} No bytecode-level issues found.", "✅".green());
    } else {
        println!(
            "\n{} {} bytecode-level finding(s):",
            "⚠️".yellow(),
            report.findings.len()
        );
        for f in &report.findings {
            let sev = match f.severity {
                WasmSeverity::Error => "ERROR".red(),
                WasmSeverity::Warning => "WARN".yellow(),
                WasmSeverity::Info => "INFO".blue(),
            };
            println!("   {} [{}] {} {}", "->".red(), f.code.bold(), sev, f.title);
            println!("      {}", f.detail);
        }
    }

    // Always remind the reader what bytecode analysis can't do; expand it when
    // asked. This is core to the issue's "documented limitations vs source mode".
    println!(
        "\n{} Source-optional mode runs a subset of the source checks.",
        "ℹ️".blue()
    );
    if args.show_limitations {
        for lim in WasmReport::limitations() {
            println!("   - {lim}");
        }
    } else {
        println!("   Re-run with {} for the full list, or see", "--show-limitations".bold());
        println!("   docs/wasm-analysis.md for the source-vs-WASM comparison.");
    }
}
