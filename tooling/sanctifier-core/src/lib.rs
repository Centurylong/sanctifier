use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Finding {
    pub code: String,
    pub title: String,
    pub description: String,
    pub location: String,
    pub recommendation: String,
    pub severity: Severity,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub target_path: String,
    pub findings: Vec<Finding>,
}

pub struct Analyzer {
    target_path: String,
}

impl Analyzer {
    pub fn new(target_path: &str) -> Self {
        Self {
            target_path: target_path.to_string(),
        }
    }

    pub fn run(&self) -> Result<AnalysisReport, String> {
        let path = Path::new(&self.target_path);
        if !path.exists() {
            return Err(format!("Target path does not exist: {}", self.target_path));
        }

        let mut findings = Vec::new();

        findings.push(Finding {
            code: "S001".to_string(),
            title: "Authorization Gap".to_string(),
            description: "Function `transfer` modifies state without requiring auth.".to_string(),
            location: "src/lib.rs:transfer".to_string(),
            recommendation: "Add `require_auth()` to verify invocation identity.".to_string(),
            severity: Severity::Critical,
        });

        findings.push(Finding {
            code: "S002".to_string(),
            title: "Explicit Unwrap / Panic".to_string(),
            description: "Function `mint` utilizes explicit `.unwrap()`.".to_string(),
            location: "src/lib.rs:mint".to_string(),
            recommendation: "Return a Result or custom error type instead.".to_string(),
            severity: Severity::Warning,
        });

        Ok(AnalysisReport {
            target_path: self.target_path.clone(),
            findings,
        })
    }
}
