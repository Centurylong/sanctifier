use crate::vulndb::{VulnDatabase, VulnEntry};
use clap::Args;
use colored::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

#[derive(Args, Debug)]
pub struct SdkCheckArgs {
    /// Path to the crate or workspace to check (must contain Cargo.lock)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,

    /// Path to a custom vulnerability database JSON file
    #[arg(long)]
    pub vuln_db: Option<PathBuf>,
}

/// A parsed constraint like `<=20.5.0`, `>=20.0.0`, or `=20.5.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
}

#[derive(Debug, Clone, Copy)]
struct Constraint {
    op: Op,
    version: (u64, u64, u64),
}

fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let mut parts = s.trim().splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn parse_constraint(token: &str) -> Option<Constraint> {
    let token = token.trim();
    for (prefix, op) in [
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("<", Op::Lt),
        (">", Op::Gt),
        ("=", Op::Eq),
    ] {
        if let Some(rest) = token.strip_prefix(prefix) {
            return Some(Constraint {
                op,
                version: parse_version(rest)?,
            });
        }
    }
    None
}

/// Parses an `affected_versions` string of the form `"soroban-sdk <=20.5.0"` or
/// `"soroban-sdk >=20.0.0 <20.5.0"` into (crate_name, constraints).
fn parse_affected_versions(spec: &str) -> Option<(String, Vec<Constraint>)> {
    let mut tokens = spec.split_whitespace();
    let name = tokens.next()?.to_string();
    let constraints: Vec<Constraint> = tokens.filter_map(parse_constraint).collect();
    if constraints.is_empty() {
        None
    } else {
        Some((name, constraints))
    }
}

fn constraint_matches(version: (u64, u64, u64), c: &Constraint) -> bool {
    match c.op {
        Op::Lt => version < c.version,
        Op::Le => version <= c.version,
        Op::Gt => version > c.version,
        Op::Ge => version >= c.version,
        Op::Eq => version == c.version,
    }
}

/// Reads `Cargo.lock` at `dir` and returns every resolved version of `crate_name`
/// found in the lockfile (a workspace can pin more than one).
fn resolved_versions(dir: &std::path::Path, crate_name: &str) -> Vec<String> {
    let lock_path = dir.join("Cargo.lock");
    let content = match fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let doc: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut versions = BTreeSet::new();
    if let Some(packages) = doc.get("package").and_then(|p| p.as_array()) {
        for pkg in packages {
            let name = pkg.get("name").and_then(|n| n.as_str());
            let version = pkg.get("version").and_then(|v| v.as_str());
            if let (Some(name), Some(version)) = (name, version) {
                if name == crate_name {
                    versions.insert(version.to_string());
                }
            }
        }
    }
    versions.into_iter().collect()
}

pub struct SdkVulnMatch<'a> {
    pub entry: &'a VulnEntry,
    pub found_version: String,
}

/// Cross-checks the `soroban-sdk` version(s) resolved in the target project's
/// `Cargo.lock` against every vulnerability database entry whose
/// `affected_versions` field references `soroban-sdk`, flagging any resolved
/// version that falls inside a known-vulnerable range.
pub fn exec(args: SdkCheckArgs) -> anyhow::Result<()> {
    let db = match &args.vuln_db {
        Some(path) => VulnDatabase::load(path)?,
        None => VulnDatabase::load_default(),
    };

    let is_json = args.format == "json";
    let versions = resolved_versions(&args.path, "soroban-sdk");

    if versions.is_empty() {
        if is_json {
            println!(
                "{}",
                serde_json::json!({
                    "success": false,
                    "error": "no soroban-sdk entry found in Cargo.lock",
                })
            );
        } else {
            eprintln!(
                "{} No `soroban-sdk` dependency found in {:?}/Cargo.lock",
                "⚠".yellow(),
                args.path
            );
        }
        std::process::exit(2);
    }

    let mut matches: Vec<SdkVulnMatch> = Vec::new();
    for entry in &db.vulnerabilities {
        let Some(spec) = &entry.affected_versions else {
            continue;
        };
        let Some((crate_name, constraints)) = parse_affected_versions(spec) else {
            continue;
        };
        if crate_name != "soroban-sdk" {
            continue;
        }
        for v in &versions {
            let Some(parsed) = parse_version(v) else {
                continue;
            };
            if constraints.iter().all(|c| constraint_matches(parsed, c)) {
                matches.push(SdkVulnMatch {
                    entry,
                    found_version: v.clone(),
                });
            }
        }
    }

    if is_json {
        let out: Vec<_> = matches
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.entry.id,
                    "severity": m.entry.severity,
                    "cvss": m.entry.cvss,
                    "resolved_version": m.found_version,
                    "recommendation": m.entry.recommendation,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "resolved_versions": versions,
                "vulnerable": out,
            }))?
        );
        return finish(&matches);
    }

    println!("{}", "🔐 soroban-sdk version audit".bold());
    println!("  Resolved version(s): {}", versions.join(", "));

    if matches.is_empty() {
        println!(
            "{} No known-vulnerable soroban-sdk version range matched.",
            "✔".green()
        );
    } else {
        for m in &matches {
            let sev_colored = match m.entry.severity.to_lowercase().as_str() {
                "critical" | "high" => m.entry.severity.red().bold(),
                "medium" => m.entry.severity.yellow().bold(),
                _ => m.entry.severity.normal(),
            };
            println!(
                "\n  {} {} — soroban-sdk {} ({})",
                "•".red(),
                m.entry.id.bold(),
                m.found_version,
                sev_colored
            );
            println!("    {}", m.entry.description.trim());
            println!("    recommendation: {}", m.entry.recommendation);
        }
    }

    finish(&matches)
}

fn finish(matches: &[SdkVulnMatch]) -> anyhow::Result<()> {
    if !matches.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
