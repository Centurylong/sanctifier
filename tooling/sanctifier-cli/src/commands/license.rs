use clap::Args;
use colored::*;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

#[derive(Args, Debug)]
pub struct LicenseArgs {
    /// Path to the crate or workspace to check
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Comma-separated list of additional SPDX license identifiers to allow
    #[arg(long, value_delimiter = ',')]
    pub allow: Vec<String>,

    /// Comma-separated list of SPDX license identifiers to explicitly deny,
    /// even if they'd otherwise be allowed
    #[arg(long, value_delimiter = ',')]
    pub deny: Vec<String>,
}

/// Permissive licenses considered compliant by default for a project that
/// ships as MIT (this workspace's own license). Copyleft licenses
/// (GPL/AGPL/LGPL/SSPL) are excluded by default since they impose obligations
/// incompatible with permissive redistribution.
const DEFAULT_ALLOWLIST: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-DFS-2016",
    "Unicode-3.0",
    "Zlib",
    "CC0-1.0",
    "MPL-2.0",
    "0BSD",
    "BSL-1.0",
];

struct Violation {
    name: String,
    version: String,
    license: String,
}

/// Runs `cargo metadata` for the target crate/workspace and flags every
/// dependency whose license doesn't clear the allowlist, or is explicitly
/// denied. `cargo metadata` (rather than parsing Cargo.toml files directly)
/// is used because license info lives in each dependency's own published
/// manifest, not the consuming project's.
pub fn exec(args: LicenseArgs) -> anyhow::Result<()> {
    let manifest_path = args.path.join("Cargo.toml");
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version=1",
            "--manifest-path",
        ])
        .arg(&manifest_path)
        .output()?;

    if !output.status.success() {
        eprintln!(
            "{} `cargo metadata` failed:\n{}",
            "❌".red(),
            String::from_utf8_lossy(&output.stderr)
        );
        std::process::exit(2);
    }

    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let packages = metadata
        .get("packages")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();

    let mut allowlist: BTreeSet<String> = DEFAULT_ALLOWLIST.iter().map(|s| s.to_string()).collect();
    for a in &args.allow {
        allowlist.insert(a.trim().to_string());
    }
    let denylist: BTreeSet<String> = args.deny.iter().map(|d| d.trim().to_string()).collect();

    let mut violations = Vec::new();
    let mut missing = Vec::new();

    for pkg in &packages {
        let name = pkg.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let version = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("");
        let license = pkg.get("license").and_then(|l| l.as_str());

        match license {
            None => missing.push((name.to_string(), version.to_string())),
            Some(license_expr) => {
                // License field can be an SPDX expression like "MIT OR Apache-2.0";
                // treat it as compliant if at least one listed license clears the
                // allowlist and none of the listed licenses are explicitly denied.
                let terms: Vec<&str> = license_expr
                    .split(|c: char| c == '/' || c.is_whitespace())
                    .filter(|t| !t.is_empty() && *t != "OR" && *t != "AND")
                    .collect();

                let denied = terms.iter().any(|t| denylist.contains(*t));
                let allowed = terms.iter().any(|t| allowlist.contains(*t));

                if denied || !allowed {
                    violations.push(Violation {
                        name: name.to_string(),
                        version: version.to_string(),
                        license: license_expr.to_string(),
                    });
                }
            }
        }
    }

    if args.format == "json" {
        println!(
            "{}",
            serde_json::json!({
                "total_packages": packages.len(),
                "violations": violations.iter().map(|v| serde_json::json!({
                    "name": v.name, "version": v.version, "license": v.license,
                })).collect::<Vec<_>>(),
                "missing_license": missing,
            })
        );
    } else {
        println!("{}", "📜 License compliance check".bold());
        println!("  Scanned {} package(s)", packages.len());

        if violations.is_empty() && missing.is_empty() {
            println!("{} All dependency licenses are compliant.", "✔".green());
        }

        for v in &violations {
            println!(
                "\n  {} {} {} — {}",
                "•".red(),
                v.name.bold(),
                v.version,
                v.license.yellow()
            );
        }

        if !missing.is_empty() {
            println!("\n{} {} package(s) with no declared license:", "⚠".yellow(), missing.len());
            for (name, version) in &missing {
                println!("    {name} {version}");
            }
        }
    }

    if !violations.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
