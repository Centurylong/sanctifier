use assert_cmd::Command;
use predicates::prelude::*;
use std::env;

#[test]
fn test_analyze_llm_explain_flag() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    let fixture_path = env::current_dir()
        .unwrap()
        .join("tests/fixtures/vulnerable_contract.rs");

    // Set a dummy LLM API URL that will fail fast (simulate no server)
    cmd.arg("analyze")
        .arg(fixture_path)
        .arg("--llm-explain")
        .env("LLM_API_URL", "http://localhost:59999/shouldfail")
        .assert()
        .success()
        .stdout(predicate::str::contains("Static analysis complete."));
    // We do not require LLM output, just that the CLI does not crash
}
