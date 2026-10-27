use serde_json::Value;

/// Canonical URI identifier for the Sanctifier v1 analysis report JSON Schema.
pub const REPORT_SCHEMA_ID: &str =
    "https://sanctifier.dev/schemas/sanctifier-report.v1.schema.json";

/// Semantic version of the Sanctifier v1 analysis report JSON Schema.
pub const REPORT_SCHEMA_VERSION: &str = "1.0.0";

/// Embedded JSON Schema (Draft 2020-12) for `sanctifier analyze --format json`.
pub const REPORT_SCHEMA_V1_JSON: &str =
    include_str!("../../../schemas/sanctifier-report.v1.schema.json");

/// Returns the parsed v1 JSON Schema as a `serde_json::Value`.
pub fn report_json_schema_v1() -> Value {
    serde_json::from_str(REPORT_SCHEMA_V1_JSON)
        .expect("embedded sanctifier-report.v1.schema.json must be valid JSON")
}

/// Validates a `sanctifier analyze --format json` payload against the v1 schema contract.
pub fn validate_report_json_v1(report: &Value) -> Result<(), Vec<String>> {
    let schema = report_json_schema_v1();
    let mut errors = Vec::new();

    let Some(root) = report.as_object() else {
        errors.push("root report value must be a JSON object".to_string());
        return Err(errors);
    };

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for req in required.iter().filter_map(Value::as_str) {
            if !root.contains_key(req) {
                errors.push(format!("missing required top-level property '{req}'"));
            }
        }
    }

    if root.get("$schema").and_then(Value::as_str) != Some(REPORT_SCHEMA_ID) {
        errors.push(format!(
            "expected '$schema' to equal '{REPORT_SCHEMA_ID}', got {:?}",
            root.get("$schema")
        ));
    }

    if root.get("schema_version").and_then(Value::as_str) != Some(REPORT_SCHEMA_VERSION) {
        errors.push(format!(
            "expected 'schema_version' to equal '{REPORT_SCHEMA_VERSION}', got {:?}",
            root.get("schema_version")
        ));
    }

    let array_fields = [
        "storage_collisions",
        "ledger_size_warnings",
        "unsafe_patterns",
        "auth_gaps",
        "panic_issues",
        "arithmetic_issues",
        "custom_rules",
        "event_issues",
        "unhandled_results",
        "upgrade_reports",
        "smt_issues",
        "vulnerability_db_matches",
        "error_codes",
    ];
    for field in array_fields {
        if let Some(val) = root.get(field) {
            if !val.is_array() {
                errors.push(format!("property '{field}' must be an array"));
            }
        }
    }

    if let Some(v) = root.get("vulnerability_db_version") {
        if !v.is_string() {
            errors.push("property 'vulnerability_db_version' must be a string".to_string());
        }
    }

    if let Some(meta) = root.get("metadata").and_then(Value::as_object) {
        for req in [
            "version",
            "timestamp",
            "project_path",
            "format",
            "schema_version",
        ] {
            if !meta.get(req).map(Value::is_string).unwrap_or(false) {
                errors.push(format!("metadata.{req} must be a string"));
            }
        }
        if meta.get("format").and_then(Value::as_str) != Some("sanctifier-ci-v1") {
            errors.push("metadata.format must equal 'sanctifier-ci-v1'".to_string());
        }
        if meta.get("schema_version").and_then(Value::as_str) != Some(REPORT_SCHEMA_VERSION) {
            errors.push(format!(
                "metadata.schema_version must equal '{REPORT_SCHEMA_VERSION}'"
            ));
        }
    } else {
        errors.push("property 'metadata' must be an object".to_string());
    }

    if let Some(baseline) = root.get("baseline").and_then(Value::as_object) {
        if !baseline
            .get("suppressed_count")
            .map(Value::is_u64)
            .unwrap_or(false)
        {
            errors.push("baseline.suppressed_count must be a non-negative integer".to_string());
        }
        if !baseline
            .get("stale_entries")
            .map(Value::is_array)
            .unwrap_or(false)
        {
            errors.push("baseline.stale_entries must be an array".to_string());
        }
    } else {
        errors.push("property 'baseline' must be an object".to_string());
    }

    if let Some(summary) = root.get("summary").and_then(Value::as_object) {
        for int_key in [
            "total_findings",
            "storage_collisions",
            "auth_gaps",
            "panic_issues",
            "arithmetic_issues",
            "size_warnings",
            "unsafe_patterns",
            "custom_rule_matches",
            "event_issues",
            "unhandled_results",
            "smt_issues",
        ] {
            if !summary.get(int_key).map(Value::is_u64).unwrap_or(false) {
                errors.push(format!("summary.{int_key} must be a non-negative integer"));
            }
        }
        for bool_key in ["has_critical", "has_high"] {
            if !summary
                .get(bool_key)
                .map(Value::is_boolean)
                .unwrap_or(false)
            {
                errors.push(format!("summary.{bool_key} must be a boolean"));
            }
        }
    } else {
        errors.push("property 'summary' must be an object".to_string());
    }

    if let Some(findings) = root.get("findings").and_then(Value::as_object) {
        for cat in [
            "storage_collisions",
            "ledger_size_warnings",
            "unsafe_patterns",
            "auth_gaps",
            "panic_issues",
            "arithmetic_issues",
            "custom_rules",
            "event_issues",
            "unhandled_results",
            "upgrade_risks",
            "smt_issues",
        ] {
            if !findings.get(cat).map(Value::is_array).unwrap_or(false) {
                errors.push(format!("findings.{cat} must be an array"));
            }
        }
    } else {
        errors.push("property 'findings' must be an object".to_string());
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_embedded_schema_is_valid_draft_2020_12() {
        let schema = report_json_schema_v1();
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert_eq!(schema["$id"], REPORT_SCHEMA_ID);
        assert_eq!(
            schema["properties"]["schema_version"]["const"],
            REPORT_SCHEMA_VERSION
        );
    }

    #[test]
    fn test_validate_report_json_v1_rejects_invalid_payload() {
        let invalid = json!({ "schema_version": "0.0.0" });
        let err = validate_report_json_v1(&invalid).unwrap_err();
        assert!(!err.is_empty());
    }
}
