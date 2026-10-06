#![allow(deprecated)]

use assert_cmd::Command;
use std::env;
use std::fs;

fn json_type_matches(expected: &str, value: &serde_json::Value) -> bool {
    match expected {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
        "number" => value.is_number(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}

fn assert_schema_matches(
    root_schema: &serde_json::Value,
    schema: &serde_json::Value,
    value: &serde_json::Value,
    path: &str,
) {
    if let Some(reference) = schema.get("$ref").and_then(serde_json::Value::as_str) {
        let pointer = reference
            .strip_prefix('#')
            .unwrap_or_else(|| panic!("{path} uses unsupported non-local schema ref {reference}"));
        let resolved = root_schema
            .pointer(pointer)
            .unwrap_or_else(|| panic!("{path} references missing schema definition {reference}"));
        assert_schema_matches(root_schema, resolved, value, path);
        return;
    }

    if let Some(expected) = schema.get("const") {
        assert_eq!(value, expected, "{path} does not match schema const");
    }

    match schema.get("type") {
        Some(serde_json::Value::String(expected_type)) => {
            assert!(
                json_type_matches(expected_type, value),
                "{path} expected JSON type {expected_type}, got {value}"
            );
        }
        Some(serde_json::Value::Array(expected_types)) => {
            let expected_types: Vec<&str> = expected_types
                .iter()
                .filter_map(serde_json::Value::as_str)
                .collect();
            assert!(
                expected_types
                    .iter()
                    .any(|expected| json_type_matches(expected, value)),
                "{path} expected one of JSON types {expected_types:?}, got {value}"
            );
        }
        Some(other) => panic!("{path} has invalid schema type declaration {other}"),
        None => {}
    }

    if let Some(object) = value.as_object() {
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object);

        if let Some(required) = schema.get("required").and_then(serde_json::Value::as_array) {
            for key in required.iter().filter_map(serde_json::Value::as_str) {
                assert!(
                    object.contains_key(key),
                    "{path} missing required property {key}"
                );
            }
        }

        if let Some(properties) = properties {
            for (key, child_schema) in properties {
                if let Some(child_value) = object.get(key) {
                    let child_path = format!("{path}.{key}");
                    assert_schema_matches(root_schema, child_schema, child_value, &child_path);
                }
            }

            if schema
                .get("additionalProperties")
                .and_then(serde_json::Value::as_bool)
                == Some(false)
            {
                for key in object.keys() {
                    assert!(
                        properties.contains_key(key),
                        "{path} has undocumented property {key}"
                    );
                }
            }
        }
    }

    if let (Some(items), Some(array)) = (schema.get("items"), value.as_array()) {
        for (index, item) in array.iter().enumerate() {
            let item_path = format!("{path}[{index}]");
            assert_schema_matches(root_schema, items, item, &item_path);
        }
    }
}

fn published_schema() -> serde_json::Value {
    let schema_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../schemas/sanctifier-ci-v1.schema.json");
    let schema_text = fs::read_to_string(&schema_path).unwrap();
    serde_json::from_str(&schema_text).unwrap()
}

#[test]
fn analyze_json_matches_published_v1_schema() {
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/valid_contract.rs");

    let output = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(fixture_path)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "sanctifier analyze failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let schema = published_schema();

    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["$id"],
        "https://github.com/Centurylong/sanctifier/blob/main/schemas/sanctifier-ci-v1.schema.json"
    );
    assert_schema_matches(&schema, &schema, &report, "$");
}

#[test]
fn finding_bearing_json_matches_published_v1_schema() {
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");

    let output = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(fixture_path)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(1),
        "finding-bearing JSON should retain the documented gate exit code; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let normalized_findings = report["findings"]
        .as_object()
        .expect("report.findings should be an object");
    assert!(
        normalized_findings
            .values()
            .filter_map(serde_json::Value::as_array)
            .any(|items| !items.is_empty()),
        "vulnerable fixture should exercise at least one normalized finding schema"
    );

    let schema = published_schema();
    assert_schema_matches(&schema, &schema, &report, "$");
}
