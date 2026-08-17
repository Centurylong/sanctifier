use clap::Args;
use colored::*;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Path to the crate or workspace to audit (must contain Cargo.lock)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Do not fail (non-zero exit) even if vulnerable advisories are found
    #[arg(long)]
    pub no_fail: bool,

    /// Only fail on advisories at or above this severity (low, medium, high, critical)
    #[arg(long, default_value = "low")]
    pub min_severity: String,
}

#[derive(Debug, Deserialize)]
struct AuditReport {
    vulnerabilities: VulnSection,
    #[serde(default)]
    warnings: std::collections::HashMap<String, Vec<AuditWarning>>,
}

#[derive(Debug, Deserialize)]
struct VulnSection {
    count: u64,
    list: Vec<AuditVuln>,
}

#[derive(Debug, Deserialize)]
struct AuditVuln {
    advisory: Advisory,
    package: Package,
    versions: PatchedVersions,
}

#[derive(Debug, Deserialize)]
struct Advisory {
    id: String,
    title: String,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Package {
    name: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct PatchedVersions {
    #[serde(default)]
    patched: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AuditWarning {
    kind: String,
    package: Package,
}

fn severity_rank(s: &str) -> u8 {
    match s.to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

/// Runs `cargo audit` against the RUSTSEC advisory database for the crate/workspace
/// at `args.path`, reporting any known-vulnerable dependency versions.
///
/// This shells out to the `cargo-audit` binary (installable via
/// `cargo install cargo-audit`) rather than re-implementing advisory parsing,
/// so the advisory data stays in sync with the upstream RUSTSEC database
/// instead of drifting from a bundled copy.
pub fn exec(args: AuditArgs) -> anyhow::Result<()> {
    let is_json = args.format == "json";

    let version_check = Command::new("cargo").args(["audit", "--version"]).output();
    if version_check.is_err()
        || !version_check
            .as_ref()
            .map(|o| o.status.success())
            .unwrap_or(false)
    {
        if is_json {
            println!(
                "{}",
                serde_json::json!({
                    "success": false,
                    "error": "cargo-audit is not installed",
                    "hint": "cargo install cargo-audit"
                })
            );
        } else {
            eprintln!(
                "{} cargo-audit is not installed.\n  Install it with: {}",
                "⚠".yellow(),
                "cargo install cargo-audit".cyan()
            );
        }
        std::process::exit(if args.no_fail { 0 } else { 2 });
    }

    let output = Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(&args.path)
        .output()?;

    // cargo-audit exits non-zero when it finds vulnerabilities; that's expected,
    // so we parse stdout regardless of the exit status rather than treating it
    // as a failure of the `cargo audit` invocation itself.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: AuditReport = match serde_json::from_str(&stdout) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{} Failed to parse cargo-audit output: {e}", "❌".red());
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
            std::process::exit(2);
        }
    };

    let min_rank = severity_rank(&args.min_severity);
    let relevant: Vec<&AuditVuln> = report
        .vulnerabilities
        .list
        .iter()
        .filter(|v| {
            let sev = v.advisory.severity.as_deref().unwrap_or("medium");
            severity_rank(sev) >= min_rank
        })
        .collect();

    if is_json {
        println!("{}", serde_json::to_string_pretty(&stdout.to_string())?);
        return finish(relevant.len(), args.no_fail);
    }

    println!("{}", "🔎 RUSTSEC advisory audit".bold());
    println!(
        "  Scanned Cargo.lock at {:?} — {} advisories found ({} at/above {})",
        args.path,
        report.vulnerabilities.count,
        relevant.len(),
        args.min_severity
    );

    if relevant.is_empty() {
        println!("{} No matching RUSTSEC advisories.", "✔".green());
    } else {
        for v in &relevant {
            let sev = v.advisory.severity.as_deref().unwrap_or("unknown");
            let sev_colored = match sev.to_lowercase().as_str() {
                "critical" | "high" => sev.red().bold(),
                "medium" => sev.yellow().bold(),
                _ => sev.normal(),
            };
            println!(
                "\n  {} {} — {} ({})",
                "•".red(),
                v.advisory.id.bold(),
                v.advisory.title,
                sev_colored
            );
            println!("    package: {} {}", v.package.name, v.package.version);
            if !v.versions.patched.is_empty() {
                println!("    patched: {}", v.versions.patched.join(", "));
            }
            if let Some(url) = &v.advisory.url {
                println!("    {}", url.dimmed());
            }
        }
    }

    let all_warnings: Vec<&AuditWarning> = report.warnings.values().flatten().collect();
    if !all_warnings.is_empty() {
        println!(
            "\n{} {} unmaintained/yanked dependency warning(s)",
            "⚠".yellow(),
            all_warnings.len()
        );
        for w in &all_warnings {
            println!(
                "    [{}] {} {}",
                w.kind, w.package.name, w.package.version
            );
        }
    }

    finish(relevant.len(), args.no_fail)
}

fn finish(vuln_count: usize, no_fail: bool) -> anyhow::Result<()> {
    if vuln_count > 0 && !no_fail {
        std::process::exit(1);
    }
    Ok(())
}
