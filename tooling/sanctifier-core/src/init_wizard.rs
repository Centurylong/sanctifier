/* Authorized Protocol Quality Assurance & Formal Invariant Verification */

pub struct InitWizardScaffold {
    pub config_content: String,
    pub ci_workflow_content: String,
    pub baseline_json_content: String,
}

impl InitWizardScaffold {
    pub fn generate_defaults(project_name: &str) -> Self {
        let config_content = format!(
            r#"[sanctifier]
version = "0.2.0"
project_name = "{project_name}"
severity_threshold = "Low"

[rules]
enabled = ["all"]
excluded = []

[ci]
fail_on_critical = true
fail_on_high = true
"#,
            project_name = project_name
        );

        let ci_workflow_content = r#"name: Sanctifier Security Audit

on:
  push:
    branches: [main, master, develop]
  pull_request:

jobs:
  audit:
    name: Formal Static Invariant Verification
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run Sanctifier Linter & Detector Suite
        run: cargo run -p sanctifier-core -- check --config .sanctify.toml
"#.to_string();

        let baseline_json_content = format!(
            r#"{{
  "project": "{project_name}",
  "generated_at": 1700000000,
  "suppressed_findings": []
}}
"#,
            project_name = project_name
        );

        Self {
            config_content,
            ci_workflow_content,
            baseline_json_content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_wizard_generation() {
        let scaffold = InitWizardScaffold::generate_defaults("soroban-token");
        assert!(scaffold.config_content.contains("soroban-token"));
        assert!(scaffold.ci_workflow_content.contains("Sanctifier Security Audit"));
        assert!(scaffold.baseline_json_content.contains("suppressed_findings"));
    }
}
