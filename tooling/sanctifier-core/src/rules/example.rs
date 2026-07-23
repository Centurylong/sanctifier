use crate::rules::Rule;

pub struct NoUnsafeBlockRule;

impl Rule for NoUnsafeBlockRule {
    fn name(&self) -> &str { "no-unsafe-blocks" }
    fn description(&self) -> &str { "Detects usage of unsafe blocks in Soroban contracts" }
    
    fn check(&self, code: &str) -> Vec<String> {
        if code.contains("unsafe {") {
            vec!["Found forbidden unsafe block".to_string()]
        } else {
            vec![]
        }
    }
}
