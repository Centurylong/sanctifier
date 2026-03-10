use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprMethodCall, File, ItemFn, ImplItemFn};

/// Represents an issue where data that doesn't need to persist forever
/// is stored in persistent storage, violating Soroban best practices.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageTypeIssue {
    /// Function where the issue was detected
    pub function_name: String,
    /// The storage key being misused
    pub key: String,
    /// Current storage type being used
    pub current_storage_type: String,
    /// Recommended storage type based on usage pattern
    pub recommended_storage_type: String,
    /// Location in code
    pub location: String,
    /// Reason for the recommendation
    pub reason: String,
    /// Severity level
    pub severity: String,
}

/// Types of data usage patterns that help determine appropriate storage type
#[derive(Debug, Clone, PartialEq)]
pub enum DataUsagePattern {
    /// Data is only used within a single transaction/function call
    Transient,
    /// Data is used across multiple calls but doesn't need to persist forever
    SessionBased,
    /// Data needs to persist across contract upgrades and indefinitely
    Permanent,
    /// Usage pattern is unclear or mixed
    Unclear,
}

/// Tracks storage usage patterns for analysis
#[derive(Debug)]
struct StorageUsageTracker {
    /// Maps storage keys to their usage patterns
    key_usage_patterns: HashMap<String, DataUsagePattern>,
    /// Maps keys to the functions where they're used
    key_function_usage: HashMap<String, HashSet<String>>,
    /// Maps keys to their storage types and locations
    key_storage_info: HashMap<String, (String, String)>, // (storage_type, location)
    /// Current function being analyzed
    current_function: Option<String>,
}

impl StorageUsageTracker {
    fn new() -> Self {
        Self {
            key_usage_patterns: HashMap::new(),
            key_function_usage: HashMap::new(),
            key_storage_info: HashMap::new(),
            current_function: None,
        }
    }

    /// Analyze a key's usage pattern based on context
    fn analyze_key_pattern(&mut self, key: &str, context: &StorageContext) {
        let pattern = match context {
            StorageContext::TemporaryData => DataUsagePattern::Transient,
            StorageContext::SessionData => DataUsagePattern::SessionBased,
            StorageContext::ConfigData => DataUsagePattern::Permanent,
            StorageContext::UserBalance => DataUsagePattern::Permanent,
            StorageContext::ContractState => DataUsagePattern::Permanent,
            StorageContext::Cache => DataUsagePattern::Transient,
            StorageContext::Unknown => DataUsagePattern::Unclear,
        };

        self.key_usage_patterns.insert(key.to_string(), pattern);
    }

    /// Record that a key is used in a specific function
    fn record_key_usage(&mut self, key: &str, function: &str) {
        self.key_function_usage
            .entry(key.to_string())
            .or_insert_with(HashSet::new)
            .insert(function.to_string());
    }

    /// Record storage type and location for a key
    fn record_storage_info(&mut self, key: &str, storage_type: &str, location: &str) {
        self.key_storage_info.insert(
            key.to_string(),
            (storage_type.to_string(), location.to_string()),
        );
    }
}

/// Context clues for determining appropriate storage type
#[derive(Debug, Clone, PartialEq)]
enum StorageContext {
    /// Temporary data that doesn't need to persist
    TemporaryData,
    /// Session-based data (user sessions, temporary state)
    SessionData,
    /// Configuration data that needs to persist
    ConfigData,
    /// User balances or critical financial data
    UserBalance,
    /// Core contract state
    ContractState,
    /// Cache data
    Cache,
    /// Unknown context
    Unknown,
}

/// AST visitor that analyzes storage usage patterns
pub struct StorageTypeVisitor {
    tracker: StorageUsageTracker,
    issues: Vec<StorageTypeIssue>,
}

impl StorageTypeVisitor {
    pub fn new() -> Self {
        Self {
            tracker: StorageUsageTracker::new(),
            issues: Vec::new(),
        }
    }

    /// Analyze storage usage and generate recommendations
    pub fn analyze_and_report(mut self) -> Vec<StorageTypeIssue> {
        // Analyze each key's usage pattern and storage type
        for (key, (current_storage, location)) in &self.tracker.key_storage_info {
            if let Some(pattern) = self.tracker.key_usage_patterns.get(key) {
                let recommended_storage = self.recommend_storage_type(pattern);
                
                // Check if current storage type is appropriate
                if !self.is_storage_appropriate(current_storage, &recommended_storage) {
                    let functions = self.tracker.key_function_usage
                        .get(key)
                        .map(|funcs| funcs.iter().next().unwrap_or(&"unknown".to_string()).clone())
                        .unwrap_or_else(|| "unknown".to_string());

                    let (reason, severity) = self.get_issue_details(pattern, current_storage, &recommended_storage);

                    self.issues.push(StorageTypeIssue {
                        function_name: functions,
                        key: key.clone(),
                        current_storage_type: current_storage.clone(),
                        recommended_storage_type: recommended_storage,
                        location: location.clone(),
                        reason,
                        severity,
                    });
                }
            }
        }

        self.issues
    }

    /// Recommend appropriate storage type based on usage pattern
    fn recommend_storage_type(&self, pattern: &DataUsagePattern) -> String {
        match pattern {
            DataUsagePattern::Transient => "Temporary".to_string(),
            DataUsagePattern::SessionBased => "Instance".to_string(),
            DataUsagePattern::Permanent => "Persistent".to_string(),
            DataUsagePattern::Unclear => "Instance".to_string(), // Conservative default
        }
    }

    /// Check if current storage type is appropriate for the pattern
    fn is_storage_appropriate(&self, current: &str, recommended: &str) -> bool {
        current == recommended || 
        // Allow Instance for Temporary (less critical)
        (current == "Instance" && recommended == "Temporary")
    }

    /// Get issue details based on the mismatch
    fn get_issue_details(&self, pattern: &DataUsagePattern, current: &str, recommended: &str) -> (String, String) {
        match (pattern, current, recommended) {
            (DataUsagePattern::Transient, "Persistent", _) => (
                "Transient data stored in persistent storage wastes resources and violates Soroban best practices".to_string(),
                "high".to_string()
            ),
            (DataUsagePattern::SessionBased, "Persistent", _) => (
                "Session-based data stored in persistent storage may cause unnecessary storage costs".to_string(),
                "medium".to_string()
            ),
            (DataUsagePattern::Permanent, "Temporary", _) => (
                "Critical data stored in temporary storage may be lost unexpectedly".to_string(),
                "high".to_string()
            ),
            _ => (
                "Storage type may not be optimal for this data usage pattern".to_string(),
                "low".to_string()
            )
        }
    }

    /// Determine storage context from key name and usage
    fn determine_context(&self, key: &str, _expr: &ExprMethodCall) -> StorageContext {
        let key_lower = key.to_lowercase();
        
        // Analyze key name patterns
        if key_lower.contains("temp") || key_lower.contains("cache") || key_lower.contains("tmp") {
            StorageContext::TemporaryData
        } else if key_lower.contains("session") || key_lower.contains("nonce") || key_lower.contains("lock") {
            StorageContext::SessionData
        } else if key_lower.contains("config") || key_lower.contains("admin") || key_lower.contains("owner") {
            StorageContext::ConfigData
        } else if key_lower.contains("balance") || key_lower.contains("allowance") || key_lower.contains("supply") {
            StorageContext::UserBalance
        } else if key_lower.contains("state") || key_lower.contains("status") {
            StorageContext::ContractState
        } else {
            StorageContext::Unknown
        }
    }

    /// Extract key string from expression
    fn extract_key_string(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Lit(lit) => {
                if let syn::Lit::Str(s) = &lit.lit {
                    Some(s.value())
                } else {
                    None
                }
            }
            Expr::Reference(r) => self.extract_key_string(&r.expr),
            Expr::Path(path) => {
                Some(quote::quote!(#path).to_string())
            }
            _ => Some(quote::quote!(#expr).to_string())
        }
    }
}

impl<'ast> Visit<'ast> for StorageTypeVisitor {
    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        let prev_fn = self.tracker.current_function.take();
        self.tracker.current_function = Some(node.sig.ident.to_string());
        visit::visit_impl_item_fn(self, node);
        self.tracker.current_function = prev_fn;
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        let prev_fn = self.tracker.current_function.take();
        self.tracker.current_function = Some(node.sig.ident.to_string());
        visit::visit_item_fn(self, node);
        self.tracker.current_function = prev_fn;
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let method_name = node.method.to_string();
        
        // Check for storage operations
        if method_name == "set" || method_name == "get" || method_name == "has" {
            let receiver_str = quote::quote!(#node.receiver).to_string();
            
            // Determine storage type from receiver
            let storage_type = if receiver_str.contains("persistent") {
                "Persistent"
            } else if receiver_str.contains("instance") {
                "Instance"
            } else if receiver_str.contains("temporary") {
                "Temporary"
            } else {
                return; // Not a recognized storage call
            };

            // Extract the key from the first argument
            if let Some(first_arg) = node.args.first() {
                if let Some(key_str) = self.extract_key_string(first_arg) {
                    let location = self.tracker.current_function
                        .as_ref()
                        .map(|f| format!("{}:{}", f, node.span().start().line))
                        .unwrap_or_default();

                    // Record storage info
                    self.tracker.record_storage_info(&key_str, storage_type, &location);

                    // Record function usage
                    if let Some(current_fn) = self.tracker.current_function.clone() {
                        self.tracker.record_key_usage(&key_str, &current_fn);
                    }

                    // Determine context and analyze pattern
                    let context = self.determine_context(&key_str, node);
                    self.tracker.analyze_key_pattern(&key_str, &context);
                }
            }
        }

        visit::visit_expr_method_call(self, node);
    }
}

/// Main analyzer function for storage type validation
pub fn analyze_storage_types(source: &str) -> Vec<StorageTypeIssue> {
    let file = match syn::parse_str::<File>(source) {
        Ok(f) => f,
        Err(_) => return vec![],
    };

    let mut visitor = StorageTypeVisitor::new();
    visitor.visit_file(&file);
    visitor.analyze_and_report()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temporary_data_in_persistent_storage() {
        let src = r#"
            #[contractimpl]
            impl TestContract {
                pub fn bad_temp_storage(env: Env) {
                    env.storage().persistent().set(&"temp_data", &123);
                    env.storage().persistent().set(&"cache_value", &456);
                }
            }
        "#;
        
        let issues = analyze_storage_types(src);
        assert!(!issues.is_empty());
        
        let temp_issue = issues.iter().find(|i| i.key.contains("temp_data")).unwrap();
        assert_eq!(temp_issue.current_storage_type, "Persistent");
        assert_eq!(temp_issue.recommended_storage_type, "Temporary");
        assert_eq!(temp_issue.severity, "high");
    }

    #[test]
    fn test_session_data_in_persistent_storage() {
        let src = r#"
            #[contractimpl]
            impl TestContract {
                pub fn session_handling(env: Env) {
                    env.storage().persistent().set(&"session_id", &"abc123");
                    env.storage().persistent().set(&"nonce", &42);
                }
            }
        "#;
        
        let issues = analyze_storage_types(src);
        assert!(!issues.is_empty());
        
        let session_issue = issues.iter().find(|i| i.key.contains("session")).unwrap();
        assert_eq!(session_issue.current_storage_type, "Persistent");
        assert_eq!(session_issue.recommended_storage_type, "Instance");
        assert_eq!(session_issue.severity, "medium");
    }

    #[test]
    fn test_critical_data_in_temporary_storage() {
        let src = r#"
            #[contractimpl]
            impl TestContract {
                pub fn bad_balance_storage(env: Env) {
                    env.storage().temporary().set(&"user_balance", &1000);
                    env.storage().temporary().set(&"total_supply", &50000);
                }
            }
        "#;
        
        let issues = analyze_storage_types(src);
        assert!(!issues.is_empty());
        
        let balance_issue = issues.iter().find(|i| i.key.contains("balance")).unwrap();
        assert_eq!(balance_issue.current_storage_type, "Temporary");
        assert_eq!(balance_issue.recommended_storage_type, "Persistent");
        assert_eq!(balance_issue.severity, "high");
    }

    #[test]
    fn test_appropriate_storage_usage() {
        let src = r#"
            #[contractimpl]
            impl TestContract {
                pub fn good_storage_usage(env: Env) {
                    env.storage().persistent().set(&"user_balance", &1000);
                    env.storage().instance().set(&"session_data", &"temp");
                    env.storage().temporary().set(&"cache_value", &123);
                }
            }
        "#;
        
        let issues = analyze_storage_types(src);
        // Should have no issues for appropriate usage
        assert!(issues.is_empty());
    }

    #[test]
    fn test_config_data_storage() {
        let src = r#"
            #[contractimpl]
            impl TestContract {
                pub fn config_management(env: Env) {
                    env.storage().temporary().set(&"admin_address", &admin);
                    env.storage().instance().set(&"config_value", &settings);
                }
            }
        "#;
        
        let issues = analyze_storage_types(src);
        assert!(!issues.is_empty());
        
        // Admin address should be persistent
        let admin_issue = issues.iter().find(|i| i.key.contains("admin")).unwrap();
        assert_eq!(admin_issue.recommended_storage_type, "Persistent");
        assert_eq!(admin_issue.severity, "high");
    }
}