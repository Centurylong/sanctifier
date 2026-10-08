//! Bounded AST detector for economic use of oracle prices before a freshness guard.
//! Tracks local quote and extracted-price aliases in source order. A guard must
//! fail fast before the first price use; a timestamp mention alone is not proof
//! of validation. This is syntactic analysis, not interprocedural dataflow proof.
use crate::finding_codes::ORACLE_STALE;
use crate::rules::{Rule, RuleViolation, Severity};
use quote::ToTokens;
use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::Visit;

pub struct OracleStalenessRule;

impl OracleStalenessRule {
    pub fn new() -> Self {
        Self
    }
}
impl Default for OracleStalenessRule {
    fn default() -> Self {
        Self::new()
    }
}
impl Rule for OracleStalenessRule {
    fn name(&self) -> &str {
        "oracle_staleness"
    }
    fn description(&self) -> &str {
        "Detects oracle-derived price consumption before a fail-fast timestamp freshness guard"
    }
    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match crate::parse_cache::parse_cached(source) {
            Some(file) => (*file).clone(),
            None => return Vec::new(),
        };
        let mut visitor = OracleVisitor { violations: Vec::new() };
        visitor.visit_file(&file);
        visitor.violations
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct OracleVisitor {
    violations: Vec<RuleViolation>,
}
impl<'ast> Visit<'ast> for OracleVisitor {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.violations
            .extend(scan_function(&node.sig.ident.to_string(), &node.block));
        syn::visit::visit_item_fn(self, node);
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.violations
            .extend(scan_function(&node.sig.ident.to_string(), &node.block));
        syn::visit::visit_impl_item_fn(self, node);
    }
}

// Only mark a named quote safe after a prior rejecting guard; a late,
// unrelated, logging-only, or non-aborting timestamp comparison is not enough.
fn scan_function(name: &str, block: &syn::Block) -> Vec<RuleViolation> {
    let mut quotes = HashSet::<String>::new();
    let mut checked = HashSet::<String>::new();
    let mut aliases = HashMap::<String, String>::new();
    let mut reported = HashSet::<String>::new();
    let mut findings = Vec::new();

    for stmt in &block.stmts {
        // A locally rebound quote starts with no freshness evidence.
        if let syn::Stmt::Local(local) = stmt {
            if let (Some(ident), Some(init)) = (binding_name(&local.pat), &local.init) {
                aliases.remove(&ident);
                if is_oracle_read(&init.expr) {
                    quotes.insert(ident.clone());
                    checked.remove(&ident);
                    reported.remove(&ident);
                } else if let syn::Expr::Path(path) = init.expr.as_ref() {
                    if let Some(origin) = path.path.get_ident() {
                        let old = origin.to_string();
                        if quotes.contains(&old) {
                            quotes.insert(ident.clone());
                            if checked.contains(&old) {
                                checked.insert(ident.clone());
                            }
                        }
                    }
                }
                for quote in &quotes {
                    if is_direct_price_field(&init.expr, quote) {
                        aliases.insert(ident.clone(), quote.clone());
                    }
                }
            }
        }

        for quote in &quotes {
            if fail_fast_freshness_guard(stmt, quote) {
                checked.insert(quote.clone());
                continue;
            }
            if !checked.contains(quote)
                && !reported.contains(quote)
                && consumes_price(stmt, quote, &aliases)
            {
                let location = format!("{}:{}", name, stmt.span().start().line);
                findings.push(
                    RuleViolation::new(
                        ORACLE_STALE,
                        Severity::Warning,
                        format!(
                            "SANCT_ORACLE_STALE: oracle quote '{}' is consumed before an \
                             explicit fail-fast timestamp freshness bound",
                            quote
                        ),
                        location,
                    )
                    .with_suggestion(
                        "Check the oracle publish_timestamp/timestamp against the current \
                         ledger clock and an approved maximum age, and abort or return an \
                         error on stale data BEFORE using price/value in arithmetic, \
                         settlement, or a return value."
                            .to_string(),
                    ),
                );
                reported.insert(quote.clone());
            }
        }
    }
    findings
}

fn binding_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(v) => Some(v.ident.to_string()),
        syn::Pat::Type(v) => binding_name(&v.pat),
        _ => None,
    }
}

fn compact<T: ToTokens>(value: &T) -> String {
    value
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

// Require both an oracle-like receiver AND a price/round-read operation.
// Generic local variables named "price" or regular token balances are not
// oracle evidence. The recognized shapes include Pyth/Chainlink-style clients.
fn is_oracle_read(expr: &syn::Expr) -> bool {
    let s = compact(expr);
    let oracle_receiver = ["oracle", "price_feed", "pricefeed", "aggregator", "chainlink", "pyth"]
        .iter()
        .any(|needle| s.contains(needle));
    let read_method = [
        ".lastprice(", ".latestprice(", ".latest_price(", ".getprice(",
        ".get_price(", ".get_round_data(", ".latestrounddata(",
        ".latest_round_data(", ".price(", ".read_price(", ".fetch_price(",
    ]
    .iter()
    .any(|needle| s.contains(needle));
    oracle_receiver && read_method
}

fn fields(quote: &str) -> [String; 2] {
    [format!("{}.price", quote.to_ascii_lowercase()), format!("{}.value", quote.to_ascii_lowercase())]
}

fn is_direct_price_field(expr: &syn::Expr, quote: &str) -> bool {
    let s = compact(expr);
    fields(quote).contains(&s)
}

// Identify whole identifiers (not a suffix inside "max_price" or "price2").
fn has_ident(s: &str, ident: &str) -> bool {
    s.match_indices(ident).any(|(pos, _)| {
        let before = s[..pos].chars().last();
        let after = s[pos + ident.len()..].chars().next();
        let boundary = |c: Option<char>| {
            !c.is_some_and(|x| x.is_ascii_alphanumeric() || x == '_')
        };
        boundary(before) && boundary(after)
    })
}

fn consumes_price(stmt: &syn::Stmt, quote: &str, aliases: &HashMap<String, String>) -> bool {
    let raw = compact(stmt);
    // Merely binding "let price = quote.price" is not yet consumption.
    // Carry that alias into later arithmetic, a return, or a value sink.
    if let syn::Stmt::Local(local) = stmt {
        if let Some(init) = &local.init {
            if is_direct_price_field(&init.expr, quote) {
                return false;
            }
        }
    }
    let direct = fields(quote).iter().any(|field| raw.contains(field));
    let extracted = aliases.iter().any(|(name, owner)| {
        owner == quote && has_ident(&raw, &name.to_ascii_lowercase())
    });
    direct || extracted
}

fn quote_timestamp_condition(text: &str, quote: &str) -> bool {
    let q = quote.to_ascii_lowercase();
    let timestamp = [
        format!("{}.timestamp", q),
        format!("{}.publish_timestamp", q),
        format!("{}.round_timestamp", q),
        format!("{}.updated_at", q),
    ];
    timestamp.iter().any(|member| text.contains(member))
        && (text.contains("now") || text.contains(".timestamp()"))
}

// Conservative recognized stale-rejection comparisons. This is not a proof
// of arbitrary custom validation helpers or conditional control-flow.
fn stale_when_true(text: &str, quote: &str) -> bool {
    if !quote_timestamp_condition(text, quote) {
        return false;
    }
    let delta = text.contains("now-")
        || text.contains(".timestamp()-")
        || text.contains(".saturating_sub(");
    let expired = delta && text.contains('>') && !text.contains("<=");
    let past = [
        format!("{}.timestamp<now-", quote),
        format!("{}.publish_timestamp<now-", quote),
        format!("{}.round_timestamp<now-", quote),
    ]
    .iter()
    .any(|form| text.contains(form));
    expired || past
}

fn fresh_when_true(text: &str, quote: &str) -> bool {
    if !quote_timestamp_condition(text, quote) {
        return false;
    }
    let delta = text.contains("now-")
        || text.contains(".timestamp()-")
        || text.contains(".saturating_sub(");
    delta && (text.contains("<=") || text.contains("<"))
}

fn exits_on_failure(block: &syn::Block) -> bool {
    match block.stmts.last() {
        Some(syn::Stmt::Expr(syn::Expr::Return(_), _)) => true,
        Some(last) => {
            let s = compact(last);
            s.starts_with("panic!(")
                || s.starts_with("return")
                || s.contains(".panic_with_error(")
                || s.contains("panic_with_error(")
        }
        None => false,
    }
}

fn fail_fast_freshness_guard(stmt: &syn::Stmt, quote: &str) -> bool {
    match stmt {
        syn::Stmt::Expr(syn::Expr::If(if_stmt), _) => {
            stale_when_true(&compact(&if_stmt.cond), quote)
                && exits_on_failure(&if_stmt.then_branch)
        }
        syn::Stmt::Expr(syn::Expr::Macro(mac), _) => {
            mac.mac.path.is_ident("assert")
                && fresh_when_true(&compact(&mac.mac.tokens), quote)
        }
        syn::Stmt::Macro(mac) => {
            mac.mac.path.is_ident("assert")
                && fresh_when_true(&compact(&mac.mac.tokens), quote)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RuleRegistry;

    #[test]
    fn flags_price_multiplication_without_a_timestamp_guard() {
        let src = r#"
            fn borrow(oracle: OracleClient, asset: Symbol, amount: i128) {
                let quote = oracle.lastprice(&asset);
                let value = amount * quote.price;
            }
        "#;
        let result = OracleStalenessRule::new().check(src);
        assert_eq!(result.len(), 1, "{result:#?}");
        assert_eq!(result[0].rule_name, ORACLE_STALE);
        assert_eq!(result[0].severity, Severity::Warning);
    }

    #[test]
    fn accepts_prior_abort_on_expired_publish_timestamp() {
        let src = r#"
            fn borrow(oracle: OracleClient, env: Env, asset: Symbol, amount: i128) {
                let quote = oracle.lastprice(&asset);
                let now = env.ledger().timestamp();
                if now - quote.publish_timestamp > MAX_AGE {
                    return Err(Error::StaleOracle);
                }
                let value = amount * quote.price;
            }
        "#;
        assert!(OracleStalenessRule::new().check(src).is_empty());
    }

    #[test]
    fn does_not_trust_late_nonaborting_or_unrelated_checks() {
        let late = r#"
            fn redeem(oracle: OracleClient, amount: i128) {
                let quote = oracle.get_price();
                let result = amount * quote.value;
                if now - quote.timestamp > max_age { return Err(Error::Stale); }
            }
        "#;
        let logging_only = r#"
            fn redeem(oracle: OracleClient, amount: i128) {
                let quote = oracle.get_price();
                if now - quote.timestamp > max_age { log_stale(); }
                let result = amount * quote.value;
            }
        "#;
        let unrelated = r#"
            fn redeem(oracle: OracleClient, amount: i128) {
                let quote = oracle.get_price();
                if now - other.timestamp > max_age { return Err(Error::Stale); }
                let result = amount * quote.value;
            }
        "#;
        for src in [late, logging_only, unrelated] {
            assert_eq!(OracleStalenessRule::new().check(src).len(), 1);
        }
    }

    #[test]
    fn follows_extracted_price_alias_and_prior_assert() {
        let unchecked = r#"
            fn redeem(price_feed: Feed, amount: i128) {
                let quote = price_feed.latest_price();
                let raw_price = quote.price;
                let value = raw_price * amount;
            }
        "#;
        let guarded = r#"
            fn redeem(price_feed: Feed, amount: i128) {
                let quote = price_feed.latest_price();
                assert!(now - quote.timestamp <= max_age);
                let raw_price = quote.price;
                let value = raw_price * amount;
            }
        "#;
        assert_eq!(OracleStalenessRule::new().check(unchecked).len(), 1);
        assert!(OracleStalenessRule::new().check(guarded).is_empty());
    }

    #[test]
    fn default_registry_and_catalog_expose_detector() {
        assert!(RuleRegistry::with_default_rules()
            .available_rules()
            .contains(&"oracle_staleness"));
        assert!(crate::finding_codes::all_finding_codes()
            .iter()
            .any(|entry| entry.code == ORACLE_STALE));
    }
}
