# Sanctifier Custom Rule Authoring API (`v1`)

Sanctifier exposes a semver-stable extension API enabling third-party security auditors, protocol teams, and community developers to write, package, and execute custom Soroban contract detectors.

---

## ðŸ›ï¸ Architecture Overview

The plugin API is structured around two core traits in `sanctifier_core::plugin::v1`:

```rust
use syn::visit::Visit;
use sanctifier_core::report::{Diagnostic, Severity};

/// Stable metadata identifying a custom detector.
#[derive(Debug, Clone)]
pub struct RuleMetadata {
    pub id: &'static str,
    pub title: &'static str,
    pub severity: Severity,
    pub description: &'static str,
    pub recommendation: &'static str,
}

/// Core interface for custom rule execution.
pub trait CustomRule: Send + Sync {
    /// Return static rule metadata.
    fn metadata(&self) -> RuleMetadata;

    /// Analyze parsed Soroban Rust AST and emit diagnostics.
    fn analyze<'ast>(&self, syntax: &'ast syn::File, ctx: &AnalysisContext) -> Vec<Diagnostic>;
}

/// Registry interface for registering custom rules.
pub trait RuleRegistry {
    fn register_rule(&mut self, rule: Box<dyn CustomRule>);
}
```

---

## ðŸ“ Example: Writing a Custom Rule

Below is a complete, production-ready example of an external rule detecting unconstrained token minting calls (`UncheckedTokenMint`):

```rust
use syn::{visit::Visit, ItemFn, ExprCall};
use sanctifier_core::plugin::v1::*;

pub struct RequireAuthorizedMintRule;

impl CustomRule for RequireAuthorizedMintRule {
    fn metadata(&self) -> RuleMetadata {
        RuleMetadata {
            id: "EXT-001",
            title: "Unchecked Token Mint Invocation",
            severity: Severity::Critical,
            description: "Detects token::Client::mint calls lacking preceding require_auth guards.",
            recommendation: "Ensure admin.require_auth() is executed before invoking token minting.",
        }
    }

    fn analyze<'ast>(&self, syntax: &'ast syn::File, _ctx: &AnalysisContext) -> Vec<Diagnostic> {
        let mut visitor = MintVisitor::default();
        visitor.visit_file(syntax);
        visitor.diagnostics
    }
}

#[derive(Default)]
struct MintVisitor {
    diagnostics: Vec<Diagnostic>,
    has_auth: bool,
}

impl<'ast> Visit<'ast> for MintVisitor {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.has_auth = false;
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        // Inspect AST expressions for require_auth vs mint
        syn::visit::visit_expr_call(self, node);
    }
}
```

---

## âš™ï¸ Configuration & Registration

Custom rules can be registered in your repository's `.sanctify.toml` manifest:

```toml
[plugins]
# Load external shared library or pre-compiled WASM rule pack
external_rules = [
  "target/release/libmy_custom_rules.so",
  "rules/community_pack.wasm"
]

[rules.EXT-001]
severity = "error"
enabled = true
```

---

## ðŸ”’ Semver Stability Guarantee

All types in `sanctifier_core::plugin::v1` are guaranteed to follow Semantic Versioning (SemVer). Breaking changes to trait signatures or AST visitor contexts will only occur in major API bumps (`v2`), ensuring community-authored detectors compile reliably across minor Sanctifier releases.