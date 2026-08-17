use anyhow::Context;
use clap::Args;
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

/// Generates a rule-module scaffold from a short plain-English spec.
///
/// This is fully offline/deterministic (no network or LLM call): it picks
/// one of a few keyword-driven templates and slots in the rule name and
/// description. See `sanctifier_core::rule_scaffold` for the generation
/// logic.
#[derive(Args, Debug)]
pub struct ScaffoldRuleArgs {
    /// Short plain-English description of the bug pattern to detect
    /// (e.g. "flags unwrap() calls on storage reads that can panic")
    pub spec: String,

    /// Rule name (accepts snake_case, kebab-case, or PascalCase; used to
    /// derive the generated struct name, the `Rule::name()` string, and the
    /// default output file name)
    pub name: String,

    /// Write the generated module to this path instead of the default
    /// `tooling/sanctifier-core/src/rules/<name>.rs`
    #[arg(long)]
    pub output: Option<PathBuf>,
}

const DEFAULT_RULES_DIR: &str = "tooling/sanctifier-core/src/rules";

pub fn exec(args: ScaffoldRuleArgs) -> anyhow::Result<()> {
    let rule_name = sanctifier_core::rule_scaffold::normalize_rule_name(&args.name);
    let struct_name = sanctifier_core::rule_scaffold::rule_struct_name(&args.name);
    let generated = sanctifier_core::rule_scaffold::generate_rule_scaffold(&args.spec, &args.name);

    let output_path = args
        .output
        .clone()
        .unwrap_or_else(|| Path::new(DEFAULT_RULES_DIR).join(format!("{rule_name}.rs")));

    write_new_file(&output_path, &generated)?;

    print_success(&output_path, &rule_name, &struct_name);

    Ok(())
}

fn write_new_file(path: &Path, content: &str) -> anyhow::Result<()> {
    if path.exists() {
        anyhow::bail!(
            "refusing to overwrite existing file: {} (pass --output to write elsewhere, or remove the existing file)",
            path.display()
        );
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory: {}", parent.display()))?;
        }
    }

    fs::write(path, content)
        .with_context(|| format!("failed to write file: {}", path.display()))?;

    Ok(())
}

fn print_success(output_path: &Path, rule_name: &str, struct_name: &str) {
    println!(
        "{} Generated rule scaffold: {}",
        "✓".green(),
        output_path.display()
    );
    println!();
    println!("{}", "This is a starting point, not a finished detector.".yellow());
    println!("Next steps:");
    println!(
        "  1. Flesh out the detection logic in {} (see the TODOs in the file)",
        output_path.display()
    );
    println!("  2. Register the rule in tooling/sanctifier-core/src/rules/mod.rs:");
    println!("       pub mod {rule_name};");
    println!("       registry.register({struct_name}::new());");
    println!(
        "  3. Add a finding code constant for `{rule_name}` in tooling/sanctifier-core/src/finding_codes.rs"
    );
    println!(
        "  4. Add a fixture under tooling/sanctifier-core/tests/fixtures/ and a snapshot test \
         (see tooling/sanctifier-core/tests/detector_snapshots.rs)"
    );
    println!("  5. Document the new rule (README / rule reference docs, as applicable)");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn exec_writes_scaffold_to_custom_output_path() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let output_path = temp_dir.path().join("my_rule.rs");

        let args = ScaffoldRuleArgs {
            spec: "flags unwrap() calls on storage reads".to_string(),
            name: "my_rule".to_string(),
            output: Some(output_path.clone()),
        };

        exec(args).expect("scaffold generation should succeed");

        let content = fs::read_to_string(&output_path).expect("generated file should exist");
        assert!(content.contains("pub struct MyRuleRule;"));
        assert!(content.contains("impl Rule for MyRuleRule"));
        // Well-formedness of the generated Rust source itself (does it parse
        // with `syn`?) is already covered by sanctifier-core's own
        // rule_scaffold tests; the CLI crate doesn't depend on `syn` directly.
    }

    #[test]
    fn exec_refuses_to_overwrite_existing_file() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let output_path = temp_dir.path().join("existing.rs");
        fs::write(&output_path, "// pre-existing content\n").expect("fixture write should succeed");

        let args = ScaffoldRuleArgs {
            spec: "anything".to_string(),
            name: "existing".to_string(),
            output: Some(output_path.clone()),
        };

        let result = exec(args);
        assert!(result.is_err(), "exec should fail when the target file already exists");

        let content = fs::read_to_string(&output_path).expect("file should still exist");
        assert_eq!(content, "// pre-existing content\n", "existing file must not be modified");
    }

    #[test]
    fn exec_creates_parent_directories() {
        let temp_dir = TempDir::new().expect("temp dir should be created");
        let output_path = temp_dir.path().join("nested").join("dir").join("rule.rs");

        let args = ScaffoldRuleArgs {
            spec: "detect persistent storage growth".to_string(),
            name: "nested-rule".to_string(),
            output: Some(output_path.clone()),
        };

        exec(args).expect("scaffold generation should succeed");
        assert!(output_path.exists());
    }
}
