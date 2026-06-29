//! End-to-end tests for `sanctifier attest` (#354): generate an attestation
//! from a clean scan, verify it round-trips, and fail cleanly below threshold.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn write(dir: &std::path::Path, name: &str, body: &str) {
    fs::write(dir.join(name), body).unwrap();
}

#[test]
fn attest_generates_and_verifies_for_a_clean_scan() {
    let tmp = tempfile::tempdir().unwrap();
    write(
        tmp.path(),
        "lib.rs",
        "pub fn add(a: u32, b: u32) -> u32 { a.saturating_add(b) }\n",
    );
    let out = tmp.path().join("attestation.json");

    // Generate: a clean scan (score 100) clears the threshold and writes an artifact.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .args([
            "attest",
            tmp.path().to_str().unwrap(),
            "--threshold",
            "90",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    let artifact = fs::read_to_string(&out).unwrap();
    assert!(artifact.contains("sanctifier-attestation-v1"));
    assert!(artifact.contains("bulletproofs-rangeproof"));
    assert!(artifact.contains("score_commitment"));
    // The exact score must NOT leak into the artifact (zero-knowledge).
    assert!(!artifact.contains("\"score\""));

    // Verify the artifact round-trips.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .args(["attest", "--verify", out.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Attestation valid"));
}

#[test]
fn attest_fails_cleanly_below_threshold() {
    let tmp = tempfile::tempdir().unwrap();
    // This contract has at least one finding, so it cannot reach a perfect 100.
    write(
        tmp.path(),
        "lib.rs",
        "pub fn boom() { panic!(\"explode\"); }\n",
    );
    let out = tmp.path().join("attestation.json");

    // Demand a perfect score: a scan with any finding must fail cleanly.
    Command::cargo_bin("sanctifier")
        .unwrap()
        .args([
            "attest",
            tmp.path().to_str().unwrap(),
            "--threshold",
            "100",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("did not pass"));

    // No artifact is produced for a failing scan.
    assert!(!out.exists());
}

#[test]
fn verify_rejects_a_tampered_artifact() {
    let tmp = tempfile::tempdir().unwrap();
    write(tmp.path(), "lib.rs", "pub fn ok() {}\n");
    let out = tmp.path().join("attestation.json");

    Command::cargo_bin("sanctifier")
        .unwrap()
        .args([
            "attest",
            tmp.path().to_str().unwrap(),
            "--threshold",
            "80",
            "--out",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();

    // Flip the threshold in the artifact: the binding no longer matches the
    // metadata, so verification must reject it.
    let tampered = fs::read_to_string(&out)
        .unwrap()
        .replace("\"threshold\": 80", "\"threshold\": 95");
    fs::write(&out, tampered).unwrap();

    Command::cargo_bin("sanctifier")
        .unwrap()
        .args(["attest", "--verify", out.to_str().unwrap()])
        .assert()
        .failure();
}
