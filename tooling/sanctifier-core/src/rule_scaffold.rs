//! Deterministic, fully offline "spec -> rule scaffold" generator.
//!
//! This is the engine behind `sanctifier scaffold-rule`: it turns a short
//! plain-English description of a bug pattern plus a rule name into a
//! ready-to-edit Rust detector module that matches the conventions used
//! across `crate::rules` (see `rules/sanct_unwrap.rs` / `rules/ledger_size.rs`
//! for the hand-written shape this mirrors).
//!
//! There is no network access and no LLM call involved -- just keyword
//! matching over the spec text to pick one of a few hand-written templates,
//! and string substitution to slot in the rule/struct names and description.
//! It will never produce a complete, correct detector; it exists purely to
//! save a contributor the boilerplate of copy-pasting an existing rule file.

use std::collections::HashSet;

/// Generates a complete Rust source file (as a `String`) implementing the
/// `Rule` trait for a new detector, based on `spec` (a short plain-English
/// description of the bug pattern to detect) and `rule_name` (used to derive
/// the struct name, the file-appropriate module name, and the string
/// returned from `Rule::name`).
///
/// The generator picks one of three built-in stub shapes by looking for
/// keywords in `spec` (case-insensitive):
///
/// - mentions of `unwrap` / `expect` / `panic` -> a `syn::visit::Visit`-based
///   method-call (+ `panic!` macro) visitor stub.
/// - mentions of `storage` / `persistent` / `instance` / `temporary` /
///   `ledger` -> a storage-key-tracking visitor stub.
/// - anything else -> a generic, empty `syn::visit::Visit` stub.
///
/// The returned source always parses as a valid `syn::File` and always
/// contains a `#[cfg(test)] mod tests` block, matching the shape of the
/// hand-written rules in `crate::rules`.
pub fn generate_rule_scaffold(spec: &str, rule_name: &str) -> String {
    let snake_name = normalize_rule_name(rule_name);
    let struct_name = rule_struct_name(rule_name);
    let visitor_name = format!("{struct_name}Visitor");

    let description_raw = fallback(&condense_whitespace(spec), FALLBACK_DESCRIPTION);
    let description_escaped = escape_rust_string(&description_raw);
    let spec_comment = fallback(&condense_whitespace(spec), FALLBACK_SPEC_COMMENT);

    let ctx = TemplateCtx {
        snake_name: &snake_name,
        struct_name: &struct_name,
        visitor_name: &visitor_name,
        description_raw: &description_raw,
        description_escaped: &description_escaped,
        spec_comment: &spec_comment,
    };

    match ScaffoldKind::detect(spec) {
        ScaffoldKind::MethodCall => method_call_scaffold(&ctx, spec),
        ScaffoldKind::Storage => storage_scaffold(&ctx),
        ScaffoldKind::Generic => generic_scaffold(&ctx),
    }
}

/// Normalizes a user-supplied rule name (snake_case, kebab-case, PascalCase,
/// "spaced words", etc) into the `snake_case` identifier used for the file
/// name, `Rule::name()`, and the `pub mod` line a contributor adds to
/// `rules/mod.rs`.
pub fn normalize_rule_name(name: &str) -> String {
    to_snake_case(name)
}

/// Derives the `FooBarRule` struct name that will be generated for a given
/// user-supplied rule name.
pub fn rule_struct_name(name: &str) -> String {
    format!("{}Rule", to_pascal_case(&normalize_rule_name(name)))
}

const FALLBACK_DESCRIPTION: &str = "TODO: describe the bug pattern this rule detects";
const FALLBACK_SPEC_COMMENT: &str = "(no spec text provided)";

fn fallback(value: &str, default: &str) -> String {
    if value.is_empty() {
        default.to_string()
    } else {
        value.to_string()
    }
}

struct TemplateCtx<'a> {
    snake_name: &'a str,
    struct_name: &'a str,
    visitor_name: &'a str,
    description_raw: &'a str,
    description_escaped: &'a str,
    spec_comment: &'a str,
}

/// Which stub shape to emit, chosen by light keyword matching over the spec
/// text. This is intentionally not NLP -- just deterministic substring
/// matching so the same spec always produces the same scaffold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScaffoldKind {
    MethodCall,
    Storage,
    Generic,
}

const METHOD_CALL_TRIGGERS: [&str; 3] = ["unwrap", "expect", "panic"];
const STORAGE_TRIGGERS: [&str; 5] = ["storage", "persistent", "instance", "temporary", "ledger"];

impl ScaffoldKind {
    fn detect(spec: &str) -> Self {
        let lower = spec.to_lowercase();
        if METHOD_CALL_TRIGGERS.iter().any(|kw| lower.contains(kw)) {
            ScaffoldKind::MethodCall
        } else if STORAGE_TRIGGERS.iter().any(|kw| lower.contains(kw)) {
            ScaffoldKind::Storage
        } else {
            ScaffoldKind::Generic
        }
    }
}

/// Replaces `@@TOKEN@@` placeholders in `template` with the given values.
/// Plain string substitution (not `format!`) so the template text can
/// contain literal `{`/`}` braces (i.e. real Rust code) without escaping.
fn render(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (token, value) in pairs {
        out = out.replace(token, value);
    }
    out
}

fn escape_rust_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn condense_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ── snake_case / PascalCase conversion ───────────────────────────────────

fn to_snake_case(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    let mut last_was_sep = true;
    let mut last_was_lower_or_digit = false;

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if !last_was_sep && last_was_lower_or_digit {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
                last_was_lower_or_digit = false;
            } else {
                out.push(ch.to_ascii_lowercase());
                last_was_lower_or_digit = true;
            }
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
            last_was_lower_or_digit = false;
        }
    }

    let trimmed = collapse_consecutive_underscores(out.trim_matches('_'));

    if trimmed.is_empty() {
        return "unnamed_rule".to_string();
    }

    if trimmed
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
    {
        format!("rule_{trimmed}")
    } else {
        trimmed
    }
}

fn collapse_consecutive_underscores(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_underscore = false;
    for ch in input.chars() {
        if ch == '_' {
            if !prev_underscore {
                out.push('_');
            }
            prev_underscore = true;
        } else {
            out.push(ch);
            prev_underscore = false;
        }
    }
    out
}

fn to_pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

// ── Scaffold templates ────────────────────────────────────────────────────

fn method_call_scaffold(ctx: &TemplateCtx<'_>, spec: &str) -> String {
    let lower = spec.to_lowercase();
    let mut methods: Vec<&str> = Vec::new();
    if lower.contains("unwrap") {
        methods.push("unwrap");
    }
    if lower.contains("expect") {
        methods.push("expect");
    }
    if methods.is_empty() {
        methods.push("unwrap");
        methods.push("expect");
    }
    // Dedup while preserving order (in case both branches above matched).
    let mut seen = HashSet::new();
    methods.retain(|m| seen.insert(*m));

    let methods_pattern = methods
        .iter()
        .map(|m| format!("\"{m}\""))
        .collect::<Vec<_>>()
        .join(" | ");

    // Make sure the generated test fixture actually exercises one of the
    // methods the visitor was told to look for, regardless of which one that
    // ended up being.
    let fixture_call = match methods[0] {
        "expect" => "expect(\\\"boom\\\")".to_string(),
        other => format!("{other}()"),
    };

    let template = r#"use crate::rules::{Rule, RuleViolation, Severity};
use syn::spanned::Spanned;
use syn::visit::Visit;
use syn::{parse_str, File};

// Auto-generated scaffold for the `@@SNAKE@@` rule (via `sanctifier scaffold-rule`).
//
// Spec: "@@SPEC_COMMENT@@"
//
// This is a STARTING POINT, not a finished detector. The heuristics below
// (method-call name + `panic!` macro matching) are a generic template, not
// an analysis of your specific pattern -- replace/extend `@@VISITOR@@` with
// real logic, tighten the violation message/suggestion text, and follow the
// registration checklist printed by `sanctifier scaffold-rule`.

/// @@DESC_RAW@@
pub struct @@STRUCT@@;

impl @@STRUCT@@ {
    pub fn new() -> Self {
        Self
    }
}

impl Default for @@STRUCT@@ {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for @@STRUCT@@ {
    fn name(&self) -> &str {
        "@@SNAKE@@"
    }

    fn description(&self) -> &str {
        "@@DESC@@"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = @@VISITOR@@ {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct @@VISITOR@@ {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for @@VISITOR@@ {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        // TODO: narrow this down to calls that actually match the spec above
        // (e.g. only inside `#[contractimpl]` entrypoints, only on specific
        // receiver types, etc). See `rules/sanct_unwrap.rs` for an example
        // that restricts itself to `#[contractimpl]` blocks.
        if matches!(method.as_str(), @@METHODS@@) {
            let line = node.method.span().start().line;
            self.violations.push(
                RuleViolation::new(
                    "@@SNAKE@@",
                    Severity::Warning,
                    format!("`{}` call may indicate the `@@SNAKE@@` pattern", method),
                    format!("line {}", line),
                )
                .with_suggestion(format!(
                    "Review whether this `{}` can be replaced with proper Result/Option handling",
                    method
                )),
            );
        }

        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.is_ident("panic") {
            let line = node.span().start().line;
            self.violations.push(
                RuleViolation::new(
                    "@@SNAKE@@",
                    Severity::Warning,
                    "`panic!` usage may indicate the `@@SNAKE@@` pattern".to_string(),
                    format!("line {}", line),
                )
                .with_suggestion(
                    "Review whether this can return a Result/Error instead of panicking"
                        .to_string(),
                ),
            );
        }

        syn::visit::visit_macro(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_expected_pattern() {
        let source = "fn example() {\n    let x: Option<u32> = None;\n    let _ = x.@@FIXTURE_CALL@@;\n}\n";
        let findings = @@STRUCT@@::new().check(source);
        assert!(
            !findings.is_empty(),
            "expected the scaffold stub to flag at least one call"
        );
    }

    #[test]
    fn ignores_unrelated_code() {
        let source = "fn example() -> u32 {\n    40 + 2\n}\n";
        let findings = @@STRUCT@@::new().check(source);
        assert!(findings.is_empty());
    }
}
"#;

    render(
        template,
        &[
            ("@@SNAKE@@", ctx.snake_name),
            ("@@STRUCT@@", ctx.struct_name),
            ("@@VISITOR@@", ctx.visitor_name),
            ("@@DESC_RAW@@", ctx.description_raw),
            ("@@DESC@@", ctx.description_escaped),
            ("@@SPEC_COMMENT@@", ctx.spec_comment),
            ("@@METHODS@@", &methods_pattern),
            ("@@FIXTURE_CALL@@", &fixture_call),
        ],
    )
}

fn storage_scaffold(ctx: &TemplateCtx<'_>) -> String {
    let template = r#"use crate::rules::{Rule, RuleViolation, Severity};
use std::collections::HashSet;
use syn::visit::Visit;
use syn::{parse_str, File};

// Auto-generated scaffold for the `@@SNAKE@@` rule (via `sanctifier scaffold-rule`).
//
// Spec: "@@SPEC_COMMENT@@"
//
// This is a STARTING POINT, not a finished detector: the heuristic below
// flags every `.set()`/`.get()`/`.remove()`/`.has()`/`.extend_ttl()` call on
// `env.storage().{persistent,instance,temporary}()` and tracks the distinct
// key expressions seen. Replace/extend `@@VISITOR@@` with real analysis of
// whether a given key is actually unbounded/unpruned (e.g. keyed by user
// input, grown inside a loop, never removed, etc), then follow the
// registration checklist printed by `sanctifier scaffold-rule`.

/// @@DESC_RAW@@
pub struct @@STRUCT@@;

impl @@STRUCT@@ {
    pub fn new() -> Self {
        Self
    }
}

impl Default for @@STRUCT@@ {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for @@STRUCT@@ {
    fn name(&self) -> &str {
        "@@SNAKE@@"
    }

    fn description(&self) -> &str {
        "@@DESC@@"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = @@VISITOR@@ {
            violations: Vec::new(),
            seen_keys: HashSet::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct @@VISITOR@@ {
    violations: Vec<RuleViolation>,
    seen_keys: HashSet<String>,
}

impl<'ast> Visit<'ast> for @@VISITOR@@ {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        if matches!(method.as_str(), "set" | "get" | "remove" | "has" | "extend_ttl") {
            let receiver_expr: &syn::Expr = &node.receiver;
            let receiver = quote::quote!(#receiver_expr).to_string();
            let touches_storage = receiver.contains("storage")
                && (receiver.contains("persistent")
                    || receiver.contains("instance")
                    || receiver.contains("temporary"));

            if touches_storage {
                let key_expr = node
                    .args
                    .first()
                    .map(|arg| quote::quote!(#arg).to_string())
                    .unwrap_or_else(|| "<no key argument>".to_string());

                // TODO: replace this bookkeeping with real analysis of
                // whether `key_expr` is unbounded (derived from user input,
                // grown inside a loop, never pruned, etc) instead of
                // flagging every storage access.
                self.seen_keys.insert(key_expr.clone());

                let line = node.method.span().start().line;
                self.violations.push(
                    RuleViolation::new(
                        "@@SNAKE@@",
                        Severity::Info,
                        format!(
                            "storage `.{}()` call with key `{}` ({} distinct key expression(s) seen so far) -- review against the `@@SNAKE@@` pattern",
                            method,
                            key_expr,
                            self.seen_keys.len()
                        ),
                        format!("line {}", line),
                    )
                    .with_suggestion(
                        "Review whether this storage key can grow unbounded and needs pruning, \
                         a bounded key derivation, or a bump/TTL policy"
                            .to_string(),
                    ),
                );
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_persistent_storage_access() {
        let source = "fn example(env: Env) {\n    env.storage().persistent().set(&1u32, &2u32);\n}\n";
        let findings = @@STRUCT@@::new().check(source);
        assert!(
            !findings.is_empty(),
            "expected the scaffold stub to flag at least one storage call"
        );
    }

    #[test]
    fn ignores_non_storage_calls() {
        let source = "fn example() {\n    let v = Vec::<u32>::new();\n    let _ = v.get(0);\n}\n";
        let findings = @@STRUCT@@::new().check(source);
        assert!(findings.is_empty());
    }
}
"#;

    render(
        template,
        &[
            ("@@SNAKE@@", ctx.snake_name),
            ("@@STRUCT@@", ctx.struct_name),
            ("@@VISITOR@@", ctx.visitor_name),
            ("@@DESC_RAW@@", ctx.description_raw),
            ("@@DESC@@", ctx.description_escaped),
            ("@@SPEC_COMMENT@@", ctx.spec_comment),
        ],
    )
}

fn generic_scaffold(ctx: &TemplateCtx<'_>) -> String {
    let template = r#"use crate::rules::{Rule, RuleViolation};
use syn::visit::Visit;
use syn::{parse_str, File};

// Auto-generated scaffold for the `@@SNAKE@@` rule (via `sanctifier scaffold-rule`).
//
// Spec: "@@SPEC_COMMENT@@"
//
// No specific keyword (unwrap/expect/panic, storage/persistent/instance/
// temporary/ledger) was recognized in the spec above, so this is a bare,
// empty-bodied `syn::visit::Visit` stub. Override the `visit_*` hook(s) that
// match the pattern you're detecting -- see `syn::visit::Visit` for the full
// list (visit_expr_call, visit_item_fn, visit_expr_binary, visit_macro, ...)
// -- and push a `RuleViolation` for each finding. Then follow the
// registration checklist printed by `sanctifier scaffold-rule`.

/// @@DESC_RAW@@
pub struct @@STRUCT@@;

impl @@STRUCT@@ {
    pub fn new() -> Self {
        Self
    }
}

impl Default for @@STRUCT@@ {
    fn default() -> Self {
        Self::new()
    }
}

impl Rule for @@STRUCT@@ {
    fn name(&self) -> &str {
        "@@SNAKE@@"
    }

    fn description(&self) -> &str {
        "@@DESC@@"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let file = match parse_str::<File>(source) {
            Ok(file) => file,
            Err(_) => return Vec::new(),
        };

        let mut visitor = @@VISITOR@@ {
            violations: Vec::new(),
        };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

struct @@VISITOR@@ {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for @@VISITOR@@ {
    // TODO: override the `visit_*` method(s) relevant to the `@@SNAKE@@`
    // pattern described above and push a `RuleViolation` onto
    // `self.violations` for each finding. Example:
    //
    //   fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
    //       // ... inspect `node` ...
    //       syn::visit::visit_expr_call(self, node);
    //   }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_runs_without_panicking() {
        let source = "fn example() {\n    let _ = 1 + 1;\n}\n";
        let findings = @@STRUCT@@::new().check(source);
        // The generated stub does not flag anything yet -- replace this
        // assertion once real detection logic is added above.
        assert!(findings.is_empty());
    }

    #[test]
    fn handles_unparseable_source_gracefully() {
        let findings = @@STRUCT@@::new().check("not valid rust {{{");
        assert!(findings.is_empty());
    }
}
"#;

    render(
        template,
        &[
            ("@@SNAKE@@", ctx.snake_name),
            ("@@STRUCT@@", ctx.struct_name),
            ("@@VISITOR@@", ctx.visitor_name),
            ("@@DESC_RAW@@", ctx.description_raw),
            ("@@DESC@@", ctx.description_escaped),
            ("@@SPEC_COMMENT@@", ctx.spec_comment),
        ],
    )
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parses(source: &str) -> syn::File {
        syn::parse_str::<syn::File>(source).unwrap_or_else(|err| {
            panic!("generated scaffold failed to parse: {err}\n---\n{source}")
        })
    }

    #[test]
    fn unwrap_keyword_picks_method_call_stub() {
        let out = generate_rule_scaffold(
            "flags .unwrap() calls on storage reads that can panic",
            "unbounded_unwrap",
        );
        parses(&out);
        assert!(out.contains("pub struct UnboundedUnwrapRule;"));
        assert!(out.contains("impl Rule for UnboundedUnwrapRule"));
        assert!(out.contains("fn visit_expr_method_call"));
        assert!(out.contains("\"unwrap\""));
        assert!(out.contains("mod tests"));
    }

    #[test]
    fn panic_keyword_picks_method_call_stub() {
        let out = generate_rule_scaffold("detect functions that can panic! at runtime", "panics");
        parses(&out);
        assert!(out.contains("fn visit_macro"));
        assert!(out.contains("is_ident(\"panic\")"));
    }

    #[test]
    fn storage_keyword_picks_storage_stub() {
        let out = generate_rule_scaffold(
            "detect persistent storage entries that grow without bound",
            "unbounded-storage-growth",
        );
        parses(&out);
        assert!(out.contains("pub struct UnboundedStorageGrowthRule;"));
        assert!(out.contains("seen_keys"));
        assert!(out.contains("HashSet"));
    }

    #[test]
    fn no_keyword_picks_generic_stub() {
        let out = generate_rule_scaffold("flags functions with more than 5 arguments", "arg_count");
        parses(&out);
        assert!(out.contains("pub struct ArgCountRule;"));
        assert!(!out.contains("visit_expr_method_call"));
        assert!(out.contains("TODO: override the `visit_*` method"));
    }

    #[test]
    fn rule_name_normalization_handles_various_casings() {
        assert_eq!(normalize_rule_name("UnboundedStorage"), "unbounded_storage");
        assert_eq!(
            normalize_rule_name("unbounded-storage-growth"),
            "unbounded_storage_growth"
        );
        assert_eq!(normalize_rule_name("  spaced Name 2 "), "spaced_name_2");
        assert_eq!(normalize_rule_name("already_snake"), "already_snake");
        assert_eq!(normalize_rule_name(""), "unnamed_rule");
        assert_eq!(normalize_rule_name("123abc"), "rule_123abc");
    }

    #[test]
    fn rule_struct_name_is_pascal_case_plus_rule_suffix() {
        assert_eq!(
            rule_struct_name("unbounded_storage"),
            "UnboundedStorageRule"
        );
        assert_eq!(rule_struct_name("view-panic"), "ViewPanicRule");
        assert_eq!(rule_struct_name(""), "UnnamedRuleRule");
    }

    #[test]
    fn spec_with_quotes_and_backslashes_still_produces_valid_source() {
        let out = generate_rule_scaffold(
            r#"detect calls like foo.bar("x\y") that look "unsafe""#,
            "quoted_spec",
        );
        parses(&out);
        assert!(out.contains("pub struct QuotedSpecRule;"));
    }

    #[test]
    fn empty_spec_and_name_still_produce_valid_source() {
        let out = generate_rule_scaffold("", "");
        parses(&out);
        assert!(out.contains("pub struct UnnamedRuleRule;"));
        assert!(out.contains(FALLBACK_DESCRIPTION));
    }

    #[test]
    fn generated_source_always_has_expected_skeleton() {
        for (spec, name) in [
            ("unwrap panics", "a"),
            ("storage persistent instance", "b"),
            ("generic thing", "c"),
        ] {
            let out = generate_rule_scaffold(spec, name);
            let file = parses(&out);
            assert!(!file.items.is_empty());
            assert!(out.contains("impl Rule for"));
            assert!(out.contains("fn as_any(&self) -> &dyn std::any::Any"));
            assert!(out.contains("#[cfg(test)]"));
            assert!(out.contains("mod tests"));
        }
    }
}
