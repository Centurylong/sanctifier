use anyhow::{bail, Context, Result};
use sanctifier_core::SanctifyConfig;
use std::fs;
use std::path::{Path, PathBuf};

const TOP_LEVEL_KEYS: &[&str] = &[
    "ignore_paths",
    "enabled_rules",
    "ledger_limit",
    "approaching_threshold",
    "strict_mode",
    "custom_rules",
];

const CUSTOM_RULE_KEYS: &[&str] = &["name", "pattern", "severity"];

/// Load the nearest .sanctify.toml from `path` upward.
///
/// Missing configuration keeps the historical default behavior. Once a
/// configuration file is found, however, read/parse/validation failures are
/// returned to the caller instead of being silently replaced by defaults.
pub(crate) fn load_config(path: &Path) -> Result<SanctifyConfig> {
    // Anchor relative spellings before ascending, preserving lexical path ancestry.
    let path = std::path::absolute(path).with_context(|| {
        format!(
            "failed to resolve Sanctifier config search path {}",
            path.display()
        )
    })?;
    let mut current = if path.is_file() {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        path.to_path_buf()
    };

    loop {
        let config_path = current.join(".sanctify.toml");
        // exists() follows symlinks, so a dangling .sanctify.toml looks absent.
        // That silently downgrades a configured scan to the default policy.
        // symlink_metadata sees the link itself and preserves read errors.
        match fs::symlink_metadata(&config_path) {
            Ok(_) => {
                let content = fs::read_to_string(&config_path).with_context(|| {
                    format!("failed to read Sanctifier config {}", config_path.display())
                })?;
                return parse_config(&content, &config_path);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to inspect Sanctifier config {}", config_path.display())
                });
            }
        }

        if !current.pop() {
            break;
        }
    }

    Ok(SanctifyConfig::default())
}

fn parse_config(content: &str, config_path: &Path) -> Result<SanctifyConfig> {
    let value: toml::Value = toml::from_str(content).map_err(|err| {
        anyhow::anyhow!(
            "invalid Sanctifier config {}: {err}",
            config_path.display()
        )
    })?;

    reject_unknown_keys(&value, config_path)?;

    let config: SanctifyConfig = toml::from_str(content).map_err(|err| {
        anyhow::anyhow!(
            "invalid Sanctifier config {}: {err}",
            config_path.display()
        )
    })?;

    validate_values(&config, config_path)?;
    Ok(config)
}

fn reject_unknown_keys(value: &toml::Value, config_path: &Path) -> Result<()> {
    let Some(table) = value.as_table() else {
        bail!(
            "invalid Sanctifier config {}: expected a TOML table",
            config_path.display()
        );
    };

    for key in table.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) {
            bail!(
                "invalid Sanctifier config {}: unknown key `{key}`; expected one of: {}",
                config_path.display(),
                TOP_LEVEL_KEYS.join(", ")
            );
        }
    }

    if let Some(rules) = table.get("custom_rules").and_then(toml::Value::as_array) {
        for (index, rule) in rules.iter().enumerate() {
            let Some(rule_table) = rule.as_table() else {
                continue;
            };
            for key in rule_table.keys() {
                if !CUSTOM_RULE_KEYS.contains(&key.as_str()) {
                    bail!(
                        "invalid Sanctifier config {}: unknown key `custom_rules[{index}].{key}`; expected one of: {}",
                        config_path.display(),
                        CUSTOM_RULE_KEYS.join(", ")
                    );
                }
            }
        }
    }

    Ok(())
}

fn validate_values(config: &SanctifyConfig, config_path: &Path) -> Result<()> {
    if config.ledger_limit == 0 {
        bail!(
            "invalid Sanctifier config {}: `ledger_limit` must be greater than 0",
            config_path.display()
        );
    }

    if config.enabled_rules.is_empty() {
        bail!(
            "invalid Sanctifier config {}: `enabled_rules` must contain at least one rule name",
            config_path.display()
        );
    }

    if !config.approaching_threshold.is_finite()
        || config.approaching_threshold <= 0.0
        || config.approaching_threshold > 1.0
    {
        bail!(
            "invalid Sanctifier config {}: `approaching_threshold` must be > 0 and <= 1",
            config_path.display()
        );
    }

    for (index, rule) in config.custom_rules.iter().enumerate() {
        if rule.name.trim().is_empty() {
            bail!(
                "invalid Sanctifier config {}: `custom_rules[{index}].name` must not be empty",
                config_path.display()
            );
        }
        if rule.pattern.is_empty() {
            bail!(
                "invalid Sanctifier config {}: `custom_rules[{index}].pattern` must not be empty",
                config_path.display()
            );
        }
        regex::Regex::new(&rule.pattern).map_err(|err| {
            anyhow::anyhow!(
                "invalid Sanctifier config {}: `custom_rules[{index}].pattern` for {:?} is not a valid regex: {err}",
                config_path.display(),
                rule.name
            )
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Result<SanctifyConfig> {
        parse_config(input, Path::new("/tmp/.sanctify.toml"))
    }

    #[cfg(unix)]
    #[test]
    fn dangling_config_symlink_is_not_silently_treated_as_absent() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        fs::create_dir(&project).unwrap();
        symlink("nonexistent-config.toml", project.join(".sanctify.toml")).unwrap();

        let error = load_config(&project).unwrap_err().to_string();
        assert!(error.contains("failed to read Sanctifier config"));
        assert!(error.contains(".sanctify.toml"));
    }

    #[test]
    fn missing_configuration_still_uses_defaults() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            load_config(root.path()).unwrap().enabled_rules,
            SanctifyConfig::default().enabled_rules
        );
    }

    #[test]
    fn valid_config_parses() {
        let config = parse(
            r#"
ignore_paths = ["target", ".git", "generated"]
enabled_rules = ["auth_gaps", "events"]
ledger_limit = 65536
approaching_threshold = 0.75
strict_mode = true

[[custom_rules]]
name = "no_forget"
pattern = "std::mem::forget"
severity = "error"
"#,
        )
        .unwrap();

        assert_eq!(config.ledger_limit, 65536);
        assert_eq!(config.approaching_threshold, 0.75);
        assert_eq!(config.custom_rules.len(), 1);
    }

    #[test]
    fn unknown_top_level_key_is_rejected() {
        let err = parse("ledger_limt = 64000").unwrap_err().to_string();
        assert!(err.contains("unknown key `ledger_limt`"));
        assert!(err.contains(".sanctify.toml"));
    }

    #[test]
    fn unknown_custom_rule_key_is_rejected() {
        let err = parse(
            r#"
[[custom_rules]]
name = "x"
pattern = "x"
severty = "warning"
"#,
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("custom_rules[0].severty"));
    }

    #[test]
    fn wrong_type_is_reported_instead_of_falling_back_to_defaults() {
        let err = parse("strict_mode = \"yes\"").unwrap_err().to_string();
        assert!(err.contains("strict_mode"));
    }

    #[test]
    fn semantic_bounds_and_regexes_are_validated() {
        let threshold_err = parse("approaching_threshold = 1.5")
            .unwrap_err()
            .to_string();
        assert!(threshold_err.contains("approaching_threshold"));

        let rules_err = parse("enabled_rules = []").unwrap_err().to_string();
        assert!(rules_err.contains("enabled_rules"));

        let regex_err = parse(
            r#"
[[custom_rules]]
name = "bad"
pattern = "["
"#,
        )
        .unwrap_err()
        .to_string();
        assert!(regex_err.contains("not a valid regex"));
    }
}
