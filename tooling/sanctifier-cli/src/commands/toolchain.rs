//! Pinned-toolchain / reproducible-build advisory (issue #754).
//!
//! Flags two common sources of non-reproducible Soroban builds:
//!   1. No `rust-toolchain.toml` (or legacy `rust-toolchain`), or one that
//!      pins a floating channel (`stable`, `nightly`, `1.85`) instead of an
//!      exact version (`1.85.0`) — the compiler used to build the WASM
//!      depends on whatever happens to be installed locally/on the runner.
//!   2. `soroban-sdk` dependencies resolved via a floating semver requirement
//!      (the Cargo default, e.g. `"20.5.0"` means `^20.5.0`) instead of an
//!      exact pin (`"=20.5.0"`) — `cargo update` can silently move the SDK
//!      version between builds of the same source.
//!
//! This is advisory only (exit code stays 0): the goal is a report a
//! developer or CI job can read, not a hard build gate.

use anyhow::Context as _;
use clap::Args;
use colored::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Args)]
pub struct ToolchainArgs {
    /// Path to the workspace/project root to audit
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// Emit results as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolchainAdvisory {
    pub check: String,
    pub message: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolchainReport {
    pub advisories: Vec<ToolchainAdvisory>,
}

pub fn exec(args: ToolchainArgs) -> anyhow::Result<()> {
    let root = args
        .path
        .canonicalize()
        .with_context(|| format!("resolving path {:?}", args.path))?;

    let mut advisories = Vec::new();
    if let Some(advisory) = check_toolchain_pin(&root) {
        advisories.push(advisory);
    }
    advisories.extend(check_sdk_pins(&root));

    if args.json {
        let report = ToolchainReport {
            advisories: advisories.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    if advisories.is_empty() {
        println!(
            "{} Toolchain and soroban-sdk versions are pinned for reproducible builds.",
            "✅".green()
        );
    } else {
        println!(
            "{} {} reproducible-build advisory(ies):\n",
            "⚠️".yellow(),
            advisories.len()
        );
        for advisory in &advisories {
            println!(
                "  {} [{}] {}",
                "•".yellow(),
                advisory.check.bold(),
                advisory.message
            );
            println!("    at {}\n", advisory.location.dimmed());
        }
        println!(
            "See {} for how to pin a toolchain and SDK version.",
            "docs/reproducible-builds.md".cyan()
        );
    }

    Ok(())
}

/// Check whether `root` pins an exact Rust toolchain version.
pub fn check_toolchain_pin(root: &Path) -> Option<ToolchainAdvisory> {
    let toml_path = root.join("rust-toolchain.toml");
    let legacy_path = root.join("rust-toolchain");

    let (path, content) = if toml_path.is_file() {
        (toml_path.clone(), fs::read_to_string(&toml_path).ok()?)
    } else if legacy_path.is_file() {
        (legacy_path.clone(), fs::read_to_string(&legacy_path).ok()?)
    } else {
        return Some(ToolchainAdvisory {
            check: "toolchain_pin".into(),
            message: "No rust-toolchain.toml (or rust-toolchain) found — the Rust compiler \
                      version used to build this project is whatever happens to be active on \
                      the developer's or CI runner's machine, so the same source can produce \
                      different WASM bytecode across builds."
                .into(),
            location: root.display().to_string(),
        });
    };

    match extract_channel(&content) {
        Some(channel) if is_exact_version(&channel) => None,
        Some(channel) => Some(ToolchainAdvisory {
            check: "toolchain_pin".into(),
            message: format!(
                "Toolchain channel '{channel}' is not an exact version pin (e.g. '1.85.0') — \
                 floating channels drift over time and break reproducible builds."
            ),
            location: path.display().to_string(),
        }),
        None => Some(ToolchainAdvisory {
            check: "toolchain_pin".into(),
            message: "rust-toolchain file present but no `channel` could be parsed from it."
                .into(),
            location: path.display().to_string(),
        }),
    }
}

/// Walk `root` for `Cargo.toml` files with a floating `soroban-sdk` requirement.
pub fn check_sdk_pins(root: &Path) -> Vec<ToolchainAdvisory> {
    let mut cargo_tomls = Vec::new();
    collect_cargo_tomls(root, &mut cargo_tomls);

    cargo_tomls
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let req = find_soroban_sdk_requirement(&content)?;
            if is_pinned_requirement(&req) {
                return None;
            }
            Some(ToolchainAdvisory {
                check: "sdk_pin".into(),
                message: format!(
                    "soroban-sdk dependency uses '{req}', a floating version requirement — \
                     `cargo update` can silently pull in a newer SDK release between builds. \
                     Pin with '=' (e.g. \"={req}\") or resolve it through a single \
                     [workspace.dependencies] entry that other crates inherit via \
                     `{{ workspace = true }}`."
                ),
                location: path.display().to_string(),
            })
        })
        .collect()
}

fn collect_cargo_tomls(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if matches!(name, "target" | "node_modules" | ".git") {
                continue;
            }
            collect_cargo_tomls(&path, out);
        } else if name == "Cargo.toml" {
            out.push(path);
        }
    }
}

fn extract_channel(content: &str) -> Option<String> {
    if let Ok(value) = content.parse::<toml::Value>() {
        if let Some(channel) = value
            .get("toolchain")
            .and_then(|t| t.get("channel"))
            .and_then(|c| c.as_str())
        {
            return Some(channel.to_string());
        }
    }
    // Legacy `rust-toolchain` files are a bare channel name, one line.
    let trimmed = content.trim();
    if !trimmed.is_empty() && !trimmed.contains('\n') {
        return Some(trimmed.to_string());
    }
    None
}

/// An exact toolchain pin is a three-part numeric version, e.g. `1.85.0`.
/// `stable`, `nightly`, `nightly-2024-01-01`, and two-part versions like
/// `1.85` all still float across at least one axis.
fn is_exact_version(channel: &str) -> bool {
    let parts: Vec<&str> = channel.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn find_soroban_sdk_requirement(content: &str) -> Option<String> {
    let value: toml::Value = content.parse().ok()?;

    if let Some(dep) = value.get("dependencies").and_then(|d| d.get("soroban-sdk")) {
        if let Some(req) = extract_version_req(dep) {
            return Some(req);
        }
    }
    value
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.get("soroban-sdk"))
        .and_then(extract_version_req)
}

fn extract_version_req(dep: &toml::Value) -> Option<String> {
    match dep {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Table(t) => {
            // `{ workspace = true }` resolves through [workspace.dependencies]
            // instead — that's checked separately, not floating at this site.
            if t.get("workspace").and_then(|v| v.as_bool()) == Some(true) {
                return None;
            }
            t.get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        }
        _ => None,
    }
}

fn is_pinned_requirement(req: &str) -> bool {
    req.trim_start().starts_with('=')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_missing_toolchain_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(check_toolchain_pin(dir.path()).is_some());
    }

    #[test]
    fn flags_floating_channel_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"stable\"\n",
        )
        .unwrap();
        let advisory = check_toolchain_pin(dir.path()).expect("stable should be flagged");
        assert_eq!(advisory.check, "toolchain_pin");
    }

    #[test]
    fn flags_two_part_version() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85\"\n",
        )
        .unwrap();
        assert!(check_toolchain_pin(dir.path()).is_some());
    }

    #[test]
    fn accepts_exact_version_pin() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("rust-toolchain.toml"),
            "[toolchain]\nchannel = \"1.85.0\"\ncomponents = [\"rustfmt\", \"clippy\"]\n",
        )
        .unwrap();
        assert!(check_toolchain_pin(dir.path()).is_none());
    }

    #[test]
    fn accepts_legacy_exact_pin() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("rust-toolchain"), "1.85.0\n").unwrap();
        assert!(check_toolchain_pin(dir.path()).is_none());
    }

    #[test]
    fn flags_floating_sdk_dependency() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nsoroban-sdk = \"20.5.0\"\n",
        )
        .unwrap();
        let advisories = check_sdk_pins(dir.path());
        assert_eq!(advisories.len(), 1);
        assert_eq!(advisories[0].check, "sdk_pin");
    }

    #[test]
    fn flags_floating_sdk_table_dependency() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nsoroban-sdk = { version = \"20.5.0\", features = [\"testutils\"] }\n",
        )
        .unwrap();
        assert_eq!(check_sdk_pins(dir.path()).len(), 1);
    }

    #[test]
    fn accepts_pinned_sdk_dependency() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nsoroban-sdk = \"=20.5.0\"\n",
        )
        .unwrap();
        assert!(check_sdk_pins(dir.path()).is_empty());
    }

    #[test]
    fn ignores_workspace_inherited_dependency() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[dependencies]\nsoroban-sdk = { workspace = true }\n",
        )
        .unwrap();
        assert!(check_sdk_pins(dir.path()).is_empty());
    }

    #[test]
    fn recurses_into_subdirectories_but_skips_target() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("contracts/my-contract");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("Cargo.toml"),
            "[dependencies]\nsoroban-sdk = \"20.5.0\"\n",
        )
        .unwrap();

        let ignored = dir.path().join("target/debug");
        fs::create_dir_all(&ignored).unwrap();
        fs::write(
            ignored.join("Cargo.toml"),
            "[dependencies]\nsoroban-sdk = \"1.0.0\"\n",
        )
        .unwrap();

        assert_eq!(check_sdk_pins(dir.path()).len(), 1);
    }
}
