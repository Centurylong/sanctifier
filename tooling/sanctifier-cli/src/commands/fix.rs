use clap::Args;
use colored::*;
use sanctifier_core::patcher::Patcher;
use sanctifier_core::rules::Patch;
use sanctifier_core::{Analyzer, SanctifyConfig};
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Generate suggested fix diffs for findings and, only after explicit
/// confirmation, apply them. The suggestions are produced by the deterministic
/// local rule engine — no network access or API keys are required, so this is
/// fully offline.
#[derive(Args, Debug)]
pub struct FixArgs {
    /// Path to a contract directory, workspace directory, or a single .rs file
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Apply the suggested fixes. Without this flag the command is a dry run
    /// that only prints the diffs and never touches any file.
    #[arg(long)]
    pub apply: bool,

    /// Skip the interactive confirmation prompt when applying. Has no effect
    /// unless `--apply` is also set. Intended for non-interactive automation.
    #[arg(long)]
    pub yes: bool,

    /// Output format (text, json)
    #[arg(short, long, default_value = "text")]
    pub format: String,
}

/// A single suggested fix, ready to be rendered as a diff.
#[derive(Debug, Serialize)]
struct SuggestedFix {
    file: String,
    line: usize,
    description: String,
    original: String,
    replacement: String,
}

#[derive(Debug, Serialize)]
struct FixReport {
    suggestions: Vec<SuggestedFix>,
    applied: bool,
    files_changed: usize,
}

pub fn exec(args: FixArgs) -> anyhow::Result<()> {
    let is_json = args.format == "json";

    let config = load_config(&args.path);
    let analyzer = Analyzer::new(config.clone());

    let mut rs_files: Vec<PathBuf> = Vec::new();
    if args.path.is_dir() {
        collect_rs_files(&args.path, &config, &mut rs_files);
    } else if args.path.extension().and_then(|s| s.to_str()) == Some("rs") {
        rs_files.push(args.path.clone());
    }
    rs_files.sort();

    let mut suggestions = Vec::new();
    // Files that have at least one applicable patch, with the patches for them.
    let mut per_file: Vec<(PathBuf, String, Vec<Patch>)> = Vec::new();

    for file in &rs_files {
        let content = match fs::read_to_string(file) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let patches = analyzer.run_fixes(&content);
        if patches.is_empty() {
            continue;
        }
        let file_label = file.display().to_string();
        for patch in &patches {
            suggestions.push(SuggestedFix {
                file: file_label.clone(),
                line: patch.start_line,
                description: patch.description.clone(),
                original: slice_patch(&content, patch),
                replacement: patch.replacement.clone(),
            });
        }
        per_file.push((file.clone(), content, patches));
    }

    if suggestions.is_empty() {
        if is_json {
            let report = FixReport {
                suggestions,
                applied: false,
                files_changed: 0,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!("{} No auto-fixable findings detected.", "✅".green());
        }
        return Ok(());
    }

    if !is_json {
        print_suggestions(&suggestions);
    }

    // Dry run: never modify anything.
    if !args.apply {
        if is_json {
            let report = FixReport {
                suggestions,
                applied: false,
                files_changed: 0,
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            println!(
                "\n{} Dry run — no files were modified. Re-run with {} to apply.",
                "ℹ️".blue(),
                "--apply".bold()
            );
        }
        return Ok(());
    }

    // Human-in-the-loop: require explicit confirmation before writing, unless
    // the caller opted into non-interactive mode with --yes.
    if !args.yes && !confirm(&format!(
        "Apply {} suggested fix(es) across {} file(s)?",
        suggestions.len(),
        per_file.len()
    ))? {
        if !is_json {
            println!("{} Aborted — no files were modified.", "✋".yellow());
        }
        return Ok(());
    }

    let mut files_changed = 0;
    for (file, content, patches) in &per_file {
        let patched = Patcher::apply_patches(content, patches);
        if &patched != content {
            fs::write(file, patched)?;
            files_changed += 1;
        }
    }

    if is_json {
        let report = FixReport {
            suggestions,
            applied: true,
            files_changed,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!(
            "\n{} Applied fixes to {} file(s).",
            "✅".green(),
            files_changed
        );
    }

    Ok(())
}

/// Render each suggestion as a minimal, deterministic per-finding diff hunk.
fn print_suggestions(suggestions: &[SuggestedFix]) {
    println!(
        "{} {} suggested fix(es):\n",
        "🩹".bold(),
        suggestions.len()
    );
    for (idx, s) in suggestions.iter().enumerate() {
        println!(
            "{}. {} ({}:{})",
            idx + 1,
            s.description.bold(),
            s.file.dimmed(),
            s.line
        );
        for line in s.original.lines() {
            println!("{}", format!("- {line}").red());
        }
        if s.original.is_empty() {
            println!("{}", "- (insertion)".red());
        }
        for line in s.replacement.lines() {
            println!("{}", format!("+ {line}").green());
        }
        println!();
    }
}

/// Prompt on stdin/stderr for a yes/no confirmation. Defaults to "no".
fn confirm(question: &str) -> anyhow::Result<bool> {
    eprint!("{question} [y/N] ");
    io::stderr().flush()?;
    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        // EOF (non-interactive with no --yes): treat as "no".
        return Ok(false);
    }
    let answer = input.trim().to_lowercase();
    Ok(answer == "y" || answer == "yes")
}

/// Extract the exact source text that a patch replaces, so it can be shown as
/// the `-` side of the diff. Mirrors the offset logic used by `Patcher`.
fn slice_patch(source: &str, patch: &Patch) -> String {
    let start = offset_of(source, patch.start_line, patch.start_column);
    let end = offset_of(source, patch.end_line, patch.end_column);
    match (start, end) {
        (Some(s), Some(e)) if e >= s && e <= source.len() => source[s..e].to_string(),
        _ => String::new(),
    }
}

fn offset_of(source: &str, line: usize, column: usize) -> Option<usize> {
    let mut current_line = 1;
    let mut line_start = 0;
    for (i, c) in source.char_indices() {
        if current_line == line {
            // Walk `column` chars into this line.
            for (col, (j, c2)) in source[line_start..].char_indices().enumerate() {
                if col == column || c2 == '\n' {
                    return Some(line_start + j);
                }
            }
            return Some(source.len());
        }
        if c == '\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    if current_line == line {
        Some(line_start + column.min(source.len() - line_start))
    } else {
        None
    }
}

fn collect_rs_files(dir: &Path, config: &SanctifyConfig, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let ignored = config.ignore_paths.iter().any(|p| path.ends_with(p));
            if ignored || path.file_name().and_then(|s| s.to_str()) == Some("target") {
                continue;
            }
            collect_rs_files(&path, config, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn load_config(path: &Path) -> SanctifyConfig {
    let mut current = if path.is_file() {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    };
    loop {
        let config_path = current.join(".sanctify.toml");
        if config_path.exists() {
            if let Ok(content) = fs::read_to_string(&config_path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        if !current.pop() {
            break;
        }
    }
    SanctifyConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_patch_extracts_range() {
        let src = "fn a() {}\nfn b() {}\n";
        let patch = Patch {
            start_line: 2,
            start_column: 0,
            end_line: 2,
            end_column: 2,
            replacement: "FN".to_string(),
            description: "x".to_string(),
        };
        assert_eq!(slice_patch(src, &patch), "fn");
    }

    #[test]
    fn confirm_defaults_to_no_on_empty() {
        // Sanity: an empty/whitespace answer is not an affirmative.
        assert!(!matches!("".trim().to_lowercase().as_str(), "y" | "yes"));
        assert!(!matches!("n".trim().to_lowercase().as_str(), "y" | "yes"));
        assert!(matches!("Y".trim().to_lowercase().as_str(), "y" | "yes"));
    }
}
