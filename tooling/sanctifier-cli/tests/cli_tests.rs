#![allow(deprecated)]
use assert_cmd::Command;
use std::env;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_cli_help() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Usage: sanctifier"));
}

#[test]
fn test_analyze_valid_contract() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/valid_contract.rs");

    cmd.arg("analyze")
        .arg(fixture_path)
        .assert()
        .success()
        .stdout(predicates::str::contains("Static analysis complete."))
        .stdout(predicates::str::contains("No ledger size issues found."))
        .stdout(predicates::str::contains(
            "No storage key collisions found.",
        ));
}

#[test]
fn test_analyze_vulnerable_contract() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");

    cmd.arg("analyze")
        .arg(fixture_path)
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Found potential Authentication Gaps!",
        ))
        .stdout(predicates::str::contains("Found explicit Panics/Unwraps!"))
        .stdout(predicates::str::contains(
            "Found unchecked Arithmetic Operations!",
        ));
}

#[test]
fn test_analyze_json_output() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/valid_contract.rs");

    let assert = cmd
        .arg("analyze")
        .arg(fixture_path)
        .arg("--format")
        .arg("json")
        .assert()
        .success();

    // JSON starts with {
    assert.stdout(predicates::str::starts_with("{"));
}

#[test]
fn test_analyze_empty_macro_heavy() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/macro_heavy.rs");

    cmd.arg("analyze")
        .arg(fixture_path)
        .assert()
        .success()
        .stdout(predicates::str::contains("Static analysis complete."));
}

#[test]
fn test_fix_dry_run_does_not_modify_file() {
    // The fix command must never touch files without --apply.
    let temp_dir = tempdir().unwrap();
    let src = temp_dir.path().join("contract.rs");
    fs::write(
        &src,
        "#[contractimpl]\nimpl C {\n    pub fn do_bad_stuff(env: Env, admin: Address) {\n        env.storage().instance().set(&String::from_slice(&env, \"admin\"), &admin);\n    }\n}\n",
    )
    .unwrap();
    let before = fs::read_to_string(&src).unwrap();

    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("fix")
        .arg(&src)
        .assert()
        .success()
        .stdout(predicates::str::contains("Dry run"));

    // File is unchanged in dry-run mode.
    assert_eq!(fs::read_to_string(&src).unwrap(), before);
}

#[test]
fn test_fix_apply_with_yes_modifies_file() {
    let temp_dir = tempdir().unwrap();
    let src = temp_dir.path().join("contract.rs");
    fs::write(
        &src,
        "#[contractimpl]\nimpl C {\n    pub fn do_bad_stuff(env: Env, admin: Address) {\n        env.storage().instance().set(&String::from_slice(&env, \"admin\"), &admin);\n    }\n}\n",
    )
    .unwrap();
    let before = fs::read_to_string(&src).unwrap();

    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("fix")
        .arg(&src)
        .arg("--apply")
        .arg("--yes")
        .assert()
        .success()
        .stdout(predicates::str::contains("Applied fixes"));

    // With --apply --yes the file is rewritten with the suggested guard.
    let after = fs::read_to_string(&src).unwrap();
    assert_ne!(after, before);
    assert!(after.contains("require_auth"));
}

#[test]
fn test_update_help() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("update")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("latest Sanctifier binary"));
}

#[test]
fn test_init_creates_sanctify_toml_in_current_directory() {
    let temp_dir = tempdir().unwrap();
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();

    cmd.current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .success();

    let config_path = temp_dir.path().join(".sanctify.toml");
    assert!(
        config_path.exists(),
        "Expected init command to create .sanctify.toml"
    );
}

#[test]
fn test_init_fails_when_config_exists_without_force() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join(".sanctify.toml");
    fs::write(&config_path, "existing content").unwrap();

    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("init")
        .assert()
        .failure();

    let content = fs::read_to_string(&config_path).unwrap();
    assert_eq!(content, "existing content");
}

#[test]
fn test_init_overwrites_when_force_is_set() {
    let temp_dir = tempdir().unwrap();
    let config_path = temp_dir.path().join(".sanctify.toml");
    fs::write(&config_path, "existing content").unwrap();

    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.current_dir(temp_dir.path())
        .arg("init")
        .arg("--force")
        .assert()
        .success();

    let content = fs::read_to_string(&config_path).unwrap();
    assert_ne!(content, "existing content");
    assert!(content.contains("ignore_paths"));
}

// ── Baseline tests ────────────────────────────────────────────────────────────

#[test]
fn test_baseline_creates_file() {
    let temp_dir = tempdir().unwrap();
    let contract = temp_dir.path().join("contract.rs");
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");
    fs::copy(&fixture, &contract).unwrap();

    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    let baseline_path = temp_dir.path().join(".sanctify-baseline.json");
    assert!(baseline_path.exists(), "baseline file should be created");

    let content = fs::read_to_string(&baseline_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert_eq!(json["version"], 1);
    assert!(json["entries"].is_array());
}

#[test]
fn test_baseline_refuses_overwrite_without_update_flag() {
    let temp_dir = tempdir().unwrap();
    let contract = temp_dir.path().join("contract.rs");
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");
    fs::copy(&fixture, &contract).unwrap();

    // First run creates the file.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    // Second run without --update should fail.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .failure();
}

#[test]
fn test_baseline_update_flag_overwrites() {
    let temp_dir = tempdir().unwrap();
    let contract = temp_dir.path().join("contract.rs");
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");
    fs::copy(&fixture, &contract).unwrap();

    // Create baseline.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    let baseline_path = temp_dir.path().join(".sanctify-baseline.json");
    let created_at_1 = {
        let content = fs::read_to_string(&baseline_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        json["created_at"].as_str().unwrap().to_string()
    };

    // Wait a tick so the timestamp differs.
    std::thread::sleep(std::time::Duration::from_millis(1100));

    // --update should overwrite.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg("--update")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    let created_at_2 = {
        let content = fs::read_to_string(&baseline_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        json["created_at"].as_str().unwrap().to_string()
    };

    assert_ne!(created_at_1, created_at_2, "baseline should be refreshed");
}

#[test]
fn test_analyze_suppresses_baselined_findings() {
    let temp_dir = tempdir().unwrap();
    let contract = temp_dir.path().join("contract.rs");
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");
    fs::copy(&fixture, &contract).unwrap();

    // Create a baseline that includes all current findings.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    // Analyze should now suppress them and mention how many were suppressed.
    let assert = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    assert.stdout(predicates::str::contains("suppressed by baseline"));
}

#[test]
fn test_analyze_no_baseline_flag_ignores_baseline() {
    let temp_dir = tempdir().unwrap();
    let contract = temp_dir.path().join("contract.rs");
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");
    fs::copy(&fixture, &contract).unwrap();

    // Create a baseline.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    // --no-baseline should still report all findings.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg("--no-baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Found potential Authentication Gaps!",
        ));
}

#[test]
fn test_analyze_json_includes_baseline_section() {
    let temp_dir = tempdir().unwrap();
    let contract = temp_dir.path().join("contract.rs");
    let fixture = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");
    fs::copy(&fixture, &contract).unwrap();

    // Create a baseline.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("baseline")
        .arg(contract.to_str().unwrap())
        .assert()
        .success();

    let output = Command::cargo_bin("sanctifier")
        .unwrap()
        .arg("analyze")
        .arg("--format")
        .arg("json")
        .arg(contract.to_str().unwrap())
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("baseline").is_some(),
        "JSON report should include baseline section"
    );
    assert!(
        json["baseline"]["suppressed_count"].as_u64().unwrap_or(0) > 0,
        "suppressed_count should be > 0 when baseline is active"
    );
}
