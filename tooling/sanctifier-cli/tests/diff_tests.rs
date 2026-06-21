use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_diff_help() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("diff").arg("--help");
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Compare findings between working tree and a git reference"))
        .stdout(predicate::str::contains("GIT_REF"))
        .stdout(predicate::str::contains("--fail-on-new"));
}

#[test]
fn test_diff_requires_git_ref() {
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("diff");
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn test_diff_invalid_git_ref() {
    // Create a temporary git repo to test with
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    // Initialize git repo
    std::process::Command::new("git")
        .arg("init")
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    // Set git config for testing
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();
        
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    // Create and commit a simple Rust file
    fs::write(repo_path.join("test.rs"), "fn main() {}")
        .unwrap();
        
    std::process::Command::new("git")
        .args(["add", "test.rs"])
        .current_dir(repo_path)
        .output()
        .unwrap();
        
    std::process::Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("diff")
        .arg("nonexistent-ref")
        .arg("--path")
        .arg(repo_path);
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Git reference 'nonexistent-ref' not found"));
}

#[test]
fn test_diff_not_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("diff")
        .arg("HEAD")
        .arg("--path")
        .arg(repo_path);
    
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Not a git repository"));
}

#[test]
fn test_diff_json_output() {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();
    
    // Initialize git repo with basic setup
    std::process::Command::new("git")
        .arg("init")
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(repo_path)
        .output()
        .unwrap();
        
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    // Create and commit initial file
    fs::write(repo_path.join("test.rs"), "fn safe_function() { println!(\"Hello\"); }")
        .unwrap();
        
    std::process::Command::new("git")
        .args(["add", "test.rs"])
        .current_dir(repo_path)
        .output()
        .unwrap();
        
    std::process::Command::new("git")
        .args(["commit", "-m", "initial commit"])
        .current_dir(repo_path)
        .output()
        .unwrap();
    
    // Modify file to add a vulnerability
    fs::write(
        repo_path.join("test.rs"),
        r#"
        fn unsafe_function() {
            let result = some_operation().unwrap(); // This will be flagged
        }
        "#
    ).unwrap();
    
    let mut cmd = Command::cargo_bin("sanctifier").unwrap();
    cmd.arg("diff")
        .arg("HEAD")
        .arg("--format")
        .arg("json")
        .arg("--path")
        .arg(repo_path);
    
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("added"))
        .stdout(predicate::str::contains("removed"))
        .stdout(predicate::str::contains("summary"));
}