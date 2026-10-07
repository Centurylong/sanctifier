use clap::Args;
use colored::Colorize;
use sanctifier_core::baseline::{save_baseline, BaselineEntry, BASELINE_FILE};
use sanctifier_core::{Analyzer, CustomRule, SanctifyConfig};
use std::fs;
use std::path::{Path, PathBuf};

use crate::commands::baseline::collect_flat_findings;
use crate::vulndb::VulnDatabase;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Force overwrite existing configuration file
    #[arg(short, long)]
    pub force: bool,
}

pub struct ConfigGenerator;

impl ConfigGenerator {
    pub fn generate_default_config() -> SanctifyConfig {
        SanctifyConfig {
            ignore_paths: vec!["target".to_string(), ".git".to_string()],
            enabled_rules: vec![
                "auth_gaps".to_string(),
                "panics".to_string(),
                "arithmetic".to_string(),
                "ledger_size".to_string(),
            ],
            ledger_limit: 64000,
            strict_mode: false,
            custom_rules: vec![
                CustomRule {
                    name: "no_unsafe_block".to_string(),
                    pattern: "unsafe\\s*\\{".to_string(),
                    severity: sanctifier_core::RuleSeverity::Error,
                },
                CustomRule {
                    name: "no_mem_forget".to_string(),
                    pattern: "std::mem::forget".to_string(),
                    severity: sanctifier_core::RuleSeverity::Warning,
                },
            ],
            approaching_threshold: 0.8,
        }
    }
}

pub const CI_WORKFLOW_PATH: &str = ".github/workflows/sanctifier.yml";

const CI_WORKFLOW: &str = r#"name: Sanctifier

on:
  pull_request:
  push:
    branches: [main]

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Sanctifier
        run: cargo install --git https://github.com/Centurylong/sanctifier sanctifier-cli
      - name: Analyze
        run: sanctifier analyze .
"#;

pub struct FileWriter;

impl FileWriter {
    pub fn config_exists(path: &Path) -> bool {
        path.join(".sanctify.toml").exists()
    }

    pub fn write_config(config: &SanctifyConfig, path: &Path) -> anyhow::Result<PathBuf> {
        let config_path = path.join(".sanctify.toml");
        let toml_string = toml::to_string_pretty(config)?;
        fs::write(&config_path, toml_string)?;
        Ok(config_path)
    }

    pub fn write_ci_workflow(path: &Path) -> anyhow::Result<PathBuf> {
        let workflow_path = path.join(CI_WORKFLOW_PATH);
        if let Some(parent) = workflow_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&workflow_path, CI_WORKFLOW)?;
        Ok(workflow_path)
    }

    pub fn write_baseline(config: &SanctifyConfig, path: &Path) -> anyhow::Result<PathBuf> {
        let analyzer = Analyzer::new(config.clone());
        let vuln_db = VulnDatabase::load_default();
        let flat = collect_flat_findings(path, &analyzer, &vuln_db, config)?;
        let entries: Vec<BaselineEntry> = flat.iter().map(BaselineEntry::from_flat).collect();
        save_baseline(path, entries)?;
        Ok(path.join(BASELINE_FILE))
    }
}

pub struct OutputFormatter;

impl OutputFormatter {
    pub fn display_artifact(label: &str, path: &Path, written: bool) {
        if written {
            println!("{} {}: {}", "✓".green(), label, path.display());
        } else {
            println!("{} {} already exists; leaving it unchanged", "•".yellow(), label);
        }
    }

    pub fn display_error(error: &anyhow::Error) {
        eprintln!("{} Failed to initialize Sanctifier", "✗".red());
        eprintln!("   Error: {}", error);
    }
}

fn read_effective_config(target_dir: &Path) -> SanctifyConfig {
    let config_path = target_dir.join(".sanctify.toml");
    fs::read_to_string(config_path)
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default()
}

pub fn exec(args: InitArgs, path: Option<PathBuf>) -> anyhow::Result<()> {
    let target_dir = match path {
        Some(p) => p,
        // Match the relative finding paths used by the generated analyze . workflow.
        None => PathBuf::from("."),
    };

    let result = (|| -> anyhow::Result<()> {
        let generated_config = ConfigGenerator::generate_default_config();
        let config_path = target_dir.join(".sanctify.toml");
        let workflow_path = target_dir.join(CI_WORKFLOW_PATH);
        let baseline_path = target_dir.join(BASELINE_FILE);

        // A fully initialized project is idempotent. Any partial scaffold is
        // ambiguous and must be repaired explicitly so stale artifacts are not
        // silently combined with newly generated ones.
        let config_exists = config_path.exists();
        let workflow_exists = workflow_path.exists();
        let baseline_exists = baseline_path.exists();
        let scaffold_complete = config_exists && workflow_exists && baseline_exists;
        let scaffold_partial = (config_exists || workflow_exists || baseline_exists) && !scaffold_complete;
        if scaffold_partial && !args.force {
            anyhow::bail!(
                "partial Sanctifier scaffold exists; use --force to repair or refresh it"
            );
        }

        let config_written = args.force || !config_path.exists();
        if config_written {
            FileWriter::write_config(&generated_config, &target_dir)?;
        }

        let workflow_written = args.force || !workflow_path.exists();
        if workflow_written {
            FileWriter::write_ci_workflow(&target_dir)?;
        }

        let baseline_written = args.force || !baseline_path.exists();
        if baseline_written {
            // Match subsequent analyze runs: use the config that is actually on disk.
            let effective_config = read_effective_config(&target_dir);
            FileWriter::write_baseline(&effective_config, &target_dir)?;
        }

        OutputFormatter::display_artifact("Configuration", &config_path, config_written);
        OutputFormatter::display_artifact("CI workflow", &workflow_path, workflow_written);
        OutputFormatter::display_artifact("Baseline", &baseline_path, baseline_written);

        Ok(())
    })();

    if let Err(error) = &result {
        OutputFormatter::display_error(error);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_generate_default_config() {
        let config = ConfigGenerator::generate_default_config();

        // Verify ignore_paths
        assert_eq!(config.ignore_paths, vec!["target", ".git"]);

        // Verify enabled_rules
        assert_eq!(
            config.enabled_rules,
            vec!["auth_gaps", "panics", "arithmetic", "ledger_size"]
        );

        // Verify ledger_limit
        assert_eq!(config.ledger_limit, 64000);

        // Verify strict_mode
        assert!(!config.strict_mode);

        // Verify approaching_threshold
        assert_eq!(config.approaching_threshold, 0.8);

        // Verify custom_rules
        assert_eq!(config.custom_rules.len(), 2);

        let rule1 = &config.custom_rules[0];
        assert_eq!(rule1.name, "no_unsafe_block");
        assert_eq!(rule1.pattern, "unsafe\\s*\\{");

        let rule2 = &config.custom_rules[1];
        assert_eq!(rule2.name, "no_mem_forget");
        assert_eq!(rule2.pattern, "std::mem::forget");
    }

    #[test]
    fn test_config_has_all_required_fields() {
        let config = ConfigGenerator::generate_default_config();

        // Ensure all required fields are present and non-empty where appropriate
        assert!(
            !config.ignore_paths.is_empty(),
            "ignore_paths should not be empty"
        );
        assert!(
            !config.enabled_rules.is_empty(),
            "enabled_rules should not be empty"
        );
        assert!(config.ledger_limit > 0, "ledger_limit should be positive");
        assert!(
            config.approaching_threshold > 0.0 && config.approaching_threshold < 1.0,
            "approaching_threshold should be between 0 and 1"
        );
    }

    #[test]
    fn test_custom_rules_have_valid_patterns() {
        let config = ConfigGenerator::generate_default_config();

        for rule in &config.custom_rules {
            assert!(
                !rule.name.is_empty(),
                "Custom rule name should not be empty"
            );
            assert!(
                !rule.pattern.is_empty(),
                "Custom rule pattern should not be empty"
            );

            // Verify patterns are valid regex
            let regex_result = regex::Regex::new(&rule.pattern);
            assert!(
                regex_result.is_ok(),
                "Pattern '{}' should be a valid regex",
                rule.pattern
            );
        }
    }

    #[test]
    fn test_config_exists_returns_false_when_no_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();

        assert!(!FileWriter::config_exists(path));
    }

    #[test]
    fn test_config_exists_returns_true_when_file_exists() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let config_path = path.join(".sanctify.toml");

        // Create the file
        fs::write(&config_path, "test content").unwrap();

        assert!(FileWriter::config_exists(path));
    }

    #[test]
    fn test_write_config_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let config = ConfigGenerator::generate_default_config();

        let result = FileWriter::write_config(&config, path);

        assert!(result.is_ok());
        let config_path = result.unwrap();
        assert!(config_path.exists());
        assert_eq!(config_path.file_name().unwrap(), ".sanctify.toml");
    }

    #[test]
    fn test_write_config_creates_valid_toml() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let config = ConfigGenerator::generate_default_config();

        let result = FileWriter::write_config(&config, path);
        assert!(result.is_ok());

        let config_path = result.unwrap();
        let content = fs::read_to_string(&config_path).unwrap();

        // Verify it's valid TOML by parsing it
        let parsed: Result<SanctifyConfig, _> = toml::from_str(&content);
        assert!(parsed.is_ok(), "Generated TOML should be parseable");
    }

    #[test]
    fn test_write_config_returns_correct_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let config = ConfigGenerator::generate_default_config();

        let result = FileWriter::write_config(&config, path);
        assert!(result.is_ok());

        let returned_path = result.unwrap();
        let expected_path = path.join(".sanctify.toml");
        assert_eq!(returned_path, expected_path);
    }

    #[test]
    fn test_exec_scaffolds_config_ci_and_baseline() {
        let temp_dir = TempDir::new().unwrap();
        let args = InitArgs { force: false };

        let result = exec(args, Some(temp_dir.path().to_path_buf()));
        assert!(result.is_ok(), "exec should succeed in empty directory");

        let config_path = temp_dir.path().join(".sanctify.toml");
        let workflow_path = temp_dir.path().join(CI_WORKFLOW_PATH);
        let baseline_path = temp_dir.path().join(BASELINE_FILE);

        assert!(config_path.exists(), "Config file should be created");
        assert!(workflow_path.exists(), "CI workflow should be created");
        assert!(baseline_path.exists(), "Baseline should be created");

        let config_content = fs::read_to_string(&config_path).unwrap();
        let parsed: Result<SanctifyConfig, _> = toml::from_str(&config_content);
        assert!(parsed.is_ok(), "Generated TOML should be parseable");

        let workflow = fs::read_to_string(&workflow_path).unwrap();
        assert!(workflow.contains("sanctifier analyze ."));

        let baseline: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&baseline_path).unwrap()).unwrap();
        assert_eq!(baseline["version"], 1);
    }

    #[test]
    fn test_exec_is_idempotent_without_force() {
        let temp_dir = TempDir::new().unwrap();

        exec(InitArgs { force: false }, Some(temp_dir.path().to_path_buf())).unwrap();

        let config_path = temp_dir.path().join(".sanctify.toml");
        let workflow_path = temp_dir.path().join(CI_WORKFLOW_PATH);
        let baseline_path = temp_dir.path().join(BASELINE_FILE);
        let before = (
            fs::read(&config_path).unwrap(),
            fs::read(&workflow_path).unwrap(),
            fs::read(&baseline_path).unwrap(),
        );

        exec(InitArgs { force: false }, Some(temp_dir.path().to_path_buf())).unwrap();

        assert_eq!(fs::read(&config_path).unwrap(), before.0);
        assert_eq!(fs::read(&workflow_path).unwrap(), before.1);
        assert_eq!(fs::read(&baseline_path).unwrap(), before.2);
    }

    #[test]
    fn test_exec_rejects_partial_scaffold_without_force() {
        let temp_dir = TempDir::new().unwrap();
        let baseline_path = temp_dir.path().join(BASELINE_FILE);
        let stale_baseline = br#"{"version":1,"findings":[]}"#;
        fs::write(&baseline_path, stale_baseline).unwrap();

        let result = exec(
            InitArgs { force: false },
            Some(temp_dir.path().to_path_buf()),
        );

        assert!(result.is_err(), "partial scaffold should require --force");
        assert!(
            !temp_dir.path().join(".sanctify.toml").exists(),
            "config must not be created during a rejected partial repair"
        );
        assert!(
            !temp_dir.path().join(CI_WORKFLOW_PATH).exists(),
            "workflow must not be created during a rejected partial repair"
        );
        assert_eq!(
            fs::read(&baseline_path).unwrap(),
            stale_baseline,
            "existing partial artifact must remain unchanged"
        );
    }

    #[test]
    fn test_exec_with_force_refreshes_scaffold() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(".sanctify.toml");
        let workflow_path = temp_dir.path().join(CI_WORKFLOW_PATH);
        let baseline_path = temp_dir.path().join(BASELINE_FILE);
        fs::create_dir_all(workflow_path.parent().unwrap()).unwrap();
        fs::write(&config_path, "existing content").unwrap();
        fs::write(&workflow_path, "existing workflow").unwrap();
        fs::write(&baseline_path, "existing baseline").unwrap();

        let result = exec(InitArgs { force: true }, Some(temp_dir.path().to_path_buf()));
        assert!(result.is_ok(), "exec should succeed with force flag");

        assert!(fs::read_to_string(&config_path).unwrap().contains("ignore_paths"));
        assert!(fs::read_to_string(&workflow_path)
            .unwrap()
            .contains("sanctifier analyze ."));
        let baseline: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&baseline_path).unwrap()).unwrap();
        assert_eq!(baseline["version"], 1);
    }
}
