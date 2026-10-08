use crate::complexity::{analyze_all_function_complexity, THRESHOLD_CYCLOMATIC};
use crate::finding_codes::CYCLOMATIC_COMPLEXITY;
use crate::rules::{Rule, RuleViolation, Severity};

/// Flags functions whose cyclomatic complexity exceeds a configured threshold.
///
/// The metric itself comes from the shared complexity module so the detector and
/// the standalone complexity report cannot drift to different branch-counting
/// rules.
pub struct CyclomaticComplexityRule {
    threshold: u32,
}

impl CyclomaticComplexityRule {
    pub fn new() -> Self {
        Self {
            threshold: THRESHOLD_CYCLOMATIC,
        }
    }

    pub fn with_threshold(threshold: u32) -> Self {
        Self { threshold }
    }

    pub fn threshold(&self) -> u32 {
        self.threshold
    }
}

impl Default for CyclomaticComplexityRule {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for CyclomaticComplexityRule {
    fn name(&self) -> &str {
        "cyclomatic_complexity"
    }

    fn description(&self) -> &str {
        "Flags functions whose cyclomatic complexity exceeds the configured threshold"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return vec![],
        };

        analyze_all_function_complexity(&file, "")
            .functions
            .into_iter()
            .filter(|metrics| metrics.cyclomatic_complexity > self.threshold)
            .map(|metrics| {
                RuleViolation::new(
                    CYCLOMATIC_COMPLEXITY,
                    Severity::Warning,
                    format!(
                        "{CYCLOMATIC_COMPLEXITY}: function `{}` has cyclomatic complexity {}, above configured threshold {}",
                        metrics.name, metrics.cyclomatic_complexity, self.threshold
                    ),
                    metrics.name,
                )
                .with_suggestion(
                    "Split branch-heavy logic into smaller helpers or simplify control flow; raise the detector threshold only when the additional complexity is intentional".to_string(),
                )
            })
            .collect()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complexity::analyze_complexity;

    const SOURCE: &str = r#"
        pub fn hotspot(value: i32) -> i32 {
            let mut score = 0;
            if value > 0 { score += 1; }
            if value > 1 { score += 1; }
            if value > 2 { score += 1; }
            score
        }
    "#;

    #[test]
    fn custom_threshold_controls_the_hotspot_boundary() {
        let strict = CyclomaticComplexityRule::with_threshold(3);
        let relaxed = CyclomaticComplexityRule::with_threshold(4);

        assert_eq!(strict.check(SOURCE).len(), 1);
        assert!(relaxed.check(SOURCE).is_empty());
    }

    #[test]
    fn nested_helpers_do_not_inflate_enclosing_function_complexity() {
        let source = r#"
            pub fn outer(value: i32) -> i32 {
                fn helper(value: i32) -> i32 {
                    if value > 0 { return 1; }
                    if value > 1 { return 2; }
                    if value > 2 { return 3; }
                    0
                }
                if value > 0 { helper(value) } else { 0 }
            }

            struct Contract;
            impl Contract {
                pub fn method(value: i32) -> i32 {
                    fn helper(value: i32) -> i32 {
                        if value > 0 { return 1; }
                        if value > 1 { return 2; }
                        0
                    }
                    if value > 0 { helper(value) } else { 0 }
                }
            }

            pub fn closure_control(value: i32) -> i32 {
                let choose = || if value > 0 { 1 } else { 0 };
                choose()
            }
        "#;
        let ast = syn::parse_file(source).unwrap();
        let metrics = analyze_complexity(&ast, "");
        let complexity = |name: &str| {
            metrics
                .functions
                .iter()
                .find(|function| function.name == name)
                .unwrap()
                .cyclomatic_complexity
        };

        assert_eq!(complexity("outer"), 2);
        assert_eq!(complexity("method"), 2);
        assert_eq!(complexity("closure_control"), 3);
        let findings = CyclomaticComplexityRule::with_threshold(2).check(source);
        // Each nested helper is analyzed on its own, never added to the
        // enclosing public function's complexity score.
        assert_eq!(findings.len(), 3);
        assert_eq!(findings.iter().filter(|f| f.location == "helper").count(), 2);
        assert!(findings.iter().any(|f| f.location == "closure_control"));
    }

    #[test]
    fn private_free_function_hotspots_are_not_lost() {
        let source = r#"
            fn private_hotspot(value: i32) -> i32 {
                let mut out = 0;
                if value > 0 { out += 1; }
                if value > 1 { out += 1; }
                if value > 2 { out += 1; }
                out
            }
            pub fn public_small(value: i32) -> i32 { value + 1 }
        "#;
        let ast = syn::parse_file(source).unwrap();
        let public_report = analyze_complexity(&ast, "");
        assert_eq!(public_report.functions.len(), 1);
        assert_eq!(public_report.functions[0].name, "public_small");

        let findings = CyclomaticComplexityRule::with_threshold(3).check(source);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].location, "private_hotspot");
    }

    #[test]
    fn malformed_source_is_ignored() {
        assert!(CyclomaticComplexityRule::new()
            .check("pub fn broken(")
            .is_empty());
    }
}
