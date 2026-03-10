use crate::CachedAnalysis;
use sanctifier_core::{Analyzer, SanctifyConfig};

pub struct RuleEngine<'a> {
    analyzer: &'a Analyzer,
    config: &'a SanctifyConfig,
}

impl<'a> RuleEngine<'a> {
    pub fn new(analyzer: &'a Analyzer, config: &'a SanctifyConfig) -> Self {
        Self { analyzer, config }
    }

    pub fn run_all(&self, content: &str, path: Option<&std::path::Path>) -> CachedAnalysis {
        let mut analysis = CachedAnalysis {
            hash: "".to_string(), // Filled by caller
            size_warnings: self.analyzer.analyze_ledger_size(content),
            unsafe_patterns: self.analyzer.analyze_unsafe_patterns(content),
            auth_gaps: self.analyzer.scan_auth_gaps(content),
            panic_issues: self.analyzer.scan_panics(content),
            arithmetic_issues: self.analyzer.scan_arithmetic_overflow(content),
            deprecated_api_issues: self.analyzer.scan_deprecated_apis(content),
            custom_rule_matches: self
                .analyzer
                .analyze_custom_rules(content, &self.config.custom_rules),
            gas_estimations: self.analyzer.scan_gas_estimation(content),
            reentrancy_issues: self.analyzer.scan_reentrancy_risks(content),
            recursion_issues: self.analyzer.scan_recursion(content),
            storage_type_issues: self.analyzer.scan_storage_type_validation(content),
        };

        if let Some(p) = path {
            let p_str = p.to_string_lossy();
            for warning in &mut analysis.size_warnings {
                warning.struct_name = format!("{}: {}", p_str, warning.struct_name);
            }
            for issue in &mut analysis.unsafe_patterns {
                issue.snippet = format!("{}: {}", p_str, issue.snippet);
            }

            for gap in &mut analysis.auth_gaps {
                *gap = format!("{}: {}", p_str, gap);
            }
            for issue in &mut analysis.panic_issues {
                issue.location = format!("{}: {}", p_str, issue.location);
            }
            for issue in &mut analysis.arithmetic_issues {
                issue.location = format!("{}: {}", p_str, issue.location);
            }
            for issue in &mut analysis.deprecated_api_issues {
                issue.location = format!("{}: {}", p_str, issue.location);
            }
            for m in &mut analysis.custom_rule_matches {
                m.snippet = format!("{}: {}", p_str, m.snippet);
            }
            for r in &mut analysis.reentrancy_issues {
                r.location = format!("{}: {}", p_str, r.location);
            }
            for r in &mut analysis.recursion_issues {
                r.location = format!("{}: {}", p_str, r.location);
            }
            for s in &mut analysis.storage_type_issues {
                s.location = format!("{}: {}", p_str, s.location);
            }
        }

        analysis
    }
}

#[cfg(test)]
mod tests;
