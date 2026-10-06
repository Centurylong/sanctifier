#![allow(deprecated)]

use assert_cmd::Command;
use std::collections::HashMap;
use std::env;
use tempfile::tempdir;

fn text_summary_fields(stdout: &str) -> HashMap<&str, &str> {
    let line = stdout
        .lines()
        .find(|line| line.starts_with("SANCTIFIER_SUMMARY "))
        .expect("analysis should emit a machine-readable summary line");

    line.split_whitespace()
        .skip(1)
        .map(|field| field.split_once('=').expect("summary fields are key=value"))
        .collect()
}

#[test]
fn text_analysis_summary_is_parseable_and_reports_process_code() {
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/valid_contract.rs");

    let output = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(fixture)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fields = text_summary_fields(&stdout);

    assert_eq!(fields["exit_code"], "0");
    fields["total_findings"].parse::<usize>().unwrap();
    fields["suppressed_findings"].parse::<usize>().unwrap();
    fields["has_critical"].parse::<bool>().unwrap();
    fields["has_high"].parse::<bool>().unwrap();
}

#[test]
fn json_analysis_summary_reports_success_code() {
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/valid_contract.rs");

    let output = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(fixture)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["summary"]["exit_code"], 0);
    assert!(json["summary"]["suppressed_findings"].is_number());
}

#[test]
fn json_analysis_gate_reports_exit_code_one() {
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");

    let output = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(fixture)
        .arg("--format")
        .arg("json")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["summary"]["exit_code"], 1);
}

#[test]
fn operational_error_uses_exit_code_one() {
    let temp_dir = tempdir().unwrap();

    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(temp_dir.path())
        .assert()
        .code(1);
}

#[test]
fn cli_usage_error_uses_exit_code_two() {
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("--definitely-not-a-sanctifier-option")
        .assert()
        .code(2);
}

#[test]
fn analysis_summary_counts_vulnerability_database_matches() {
    let temp_dir = tempdir().unwrap();
    let fixture = temp_dir.path().join("contract.rs");
    let database = temp_dir.path().join("vulnerability-db.json");
    std::fs::write(&fixture, "// SUMMARY_DATABASE_ONLY\npub fn hello() {}\n").unwrap();
    let entry = serde_json::json!({
        "id": "SUMMARY-DB-001",
        "name": "Summary database fixture",
        "description": "A synthetic match used to check summary accounting.",
        "severity": "low",
        "category": "test",
        "pattern": "SUMMARY_DATABASE_ONLY",
        "recommendation": "Fixture only."
    });

    for format in ["text", "json"] {
        let mut totals = Vec::new();
        for include_match in [false, true] {
            let entries = if include_match {
                vec![entry.clone()]
            } else {
                Vec::new()
            };
            let db = serde_json::json!({
                "version": "test",
                "last_updated": "2026-10-06",
                "description": "Summary accounting fixture",
                "vulnerabilities": entries
            });
            std::fs::write(&database, serde_json::to_vec(&db).unwrap()).unwrap();

            let output = Command::cargo_bin("sanctifier")
                .unwrap()
                .arg("analyze")
                .arg(&fixture)
                .arg("--format")
                .arg(format)
                .arg("--vuln-db")
                .arg(&database)
                .arg("--no-baseline")
                .output()
                .unwrap();

            assert_eq!(output.status.code(), Some(0));
            let total = if format == "json" {
                let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
                assert_eq!(
                    json["vulnerability_db_matches"].as_array().unwrap().len(),
                    if include_match { 1 } else { 0 }
                );
                json["summary"]["total_findings"].as_u64().unwrap() as usize
            } else {
                let stdout = String::from_utf8(output.stdout).unwrap();
                if include_match {
                    assert!(stdout.contains("SUMMARY-DB-001"));
                }
                text_summary_fields(&stdout)["total_findings"]
                    .parse::<usize>()
                    .unwrap()
            };
            totals.push(total);
        }
        assert_eq!(
            totals[1],
            totals[0] + 1,
            "{format} summary omitted a database match"
        );
    }
}
