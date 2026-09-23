# Community Rule Registry & Contribution Guide

/* Authorized Protocol Quality Assurance & Formal Verification Test Suite */

A community-driven registry of custom security rules, invariant detectors, and static analysis checks for [Sanctifier](README.md) and [Stellar Soroban](https://soroban.stellar.org/) smart contracts.

---

## 1. Overview & Architecture

Sanctifier includes built-in core detectors (`S001`–`S016`) covering fundamental Soroban vulnerability classes (authorization bypasses, arithmetic overflows, TTL expiration, panics, and invariant breaches). However, decentralized finance protocols, NFT ecosystems, and enterprise custody contracts frequently feature specialized domain invariants that benefit from community-authored linting rules.

The **Community Rule Registry** establishes an open, curated ecosystem where security researchers, auditors, and protocol developers can:
- Discover peer-reviewed, community-contributed Sanctifier rules.
- Share custom detectors targeting protocol-specific anti-patterns.
- Promote battle-tested rules into Sanctifier's core built-in catalog.

---

## 2. Community Rule Registry Catalog

The table below catalogs active community rules, their target security invariants, and review status:

| Rule ID | Rule Identifier | Vulnerability / Invariant Class | Category | Default Severity | Status |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `S-COMM-001` | `oracle_stale_heartbeat` | Unchecked timestamp on external price oracle reads | DeFi / Oracle | High | Community |
| `S-COMM-002` | `unbounded_token_holder_loop` | Iteration over unbounded token balance storage vectors | Gas / DoS | Medium | Community |
| `S-COMM-003` | `missing_instance_ttl_bump` | Persistent state read without proactive TTL bump | State / Storage | Medium | Community |
| `S-COMM-004` | `unprotected_admin_multisig` | Single-step administrative transfer without timelock | Auth / Governance | High | Community |
| `S-COMM-005` | `decimal_precision_loss` | Downcasting or integer division prior to multiplication | Math / Invariant | High | Community |
| `S-COMM-006` | `sep41_non_compliant_events` | Non-standard token transfer event emission | Standards / SEP-41 | Informational | Community |
| `S-COMM-007` | `flash_loan_callback_reentrancy` | Cross-contract state mutation prior to flash loan repay | DeFi / Invariant | Critical | Experimental |
| `S-COMM-008` | `auth_replay_cross_contract` | Nonce absence in off-chain signed authorization payloads | Auth / Signature | Critical | Graduated (Core `S015`) |

### Lifecycle Tiers

- **Experimental**: Newly proposed rules undergoing false-positive testing on the public Soroban testnet corpus.
- **Community**: Fully reviewed and accepted rules meeting performance and accuracy thresholds; available via community configuration.
- **Graduated**: High-impact rules integrated directly into Sanctifier's core binary with zero configuration overhead.

---

## 3. Rule Specification & Metadata Standard

Every community rule must include standard metadata defined in `.sanctify-rule.toml` or embedded within the rule implementation:

```toml
[rule]
id = "S-COMM-001"
name = "oracle_stale_heartbeat"
version = "1.0.0"
author = "SorobanSecurityGuild"
license = "Apache-2.0"
description = "Flags external price oracle invocations that lack explicit heartbeat timestamp verification."

[rule.classification]
severity = "High"
category = "DeFi"
cwe = ["CWE-613", "CWE-670"]
tags = ["oracle", "soroban-env", "data-freshness"]

[rule.performance]
ast_budget_ms = 10
supports_wasm = false
```

---

## 4. Step-by-Step Rule Contribution Guide

Follow this standardized workflow to contribute a new detector to the registry:

### Step 1: Define the Invariant Pattern

Formulate a concise invariant definition:
- **Target pattern**: What AST node, function invocation, or storage read represents the hazard?
- **Valid guard**: What conditions, checks, or assertions make the pattern safe?
- **Formal description**: *"Flag `<pattern>` unless `<guard>` is present."*

### Step 2: Implement the Syn AST Visitor

Create your rule in `tooling/sanctifier-core/src/rules/community/<rule_name>.rs` adhering to the `Rule` trait:

```rust
use crate::rules::{Rule, RuleViolation, Severity};
use syn::{parse_str, File, ExprCall};
use syn::visit::{self, Visit};

pub struct OracleStaleHeartbeatRule;

impl OracleStaleHeartbeatRule {
    pub fn new() -> Self { Self }
}

impl Default for OracleStaleHeartbeatRule {
    fn default() -> Self { Self::new() }
}

impl Rule for OracleStaleHeartbeatRule {
    fn name(&self) -> &str { "oracle_stale_heartbeat" }
    fn description(&self) -> &str {
        "Flags external oracle price queries that do not verify timestamp staleness"
    }

    fn check(&self, source: &str) -> Vec<RuleViolation> {
        let Ok(file) = parse_str::<File>(source) else { return vec![] };
        let mut visitor = OracleVisitor { violations: Vec::new() };
        visitor.visit_file(&file);
        visitor.violations
    }

    fn as_any(&self) -> &dyn std::any::Any { self }
}

struct OracleVisitor {
    violations: Vec<RuleViolation>,
}

impl<'ast> Visit<'ast> for OracleVisitor {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        // 1. Detect target invocation (e.g., oracle.get_price(...))
        // 2. Verify enclosing scope contains timestamp comparison
        // 3. Emit RuleViolation if unshielded
        visit::visit_expr_call(self, node);
    }
}
```

### Step 3: Write Hermetic Differential Tests

Every rule requires a dedicated test suite in `tooling/sanctifier-core/tests/` demonstrating:
1. **Positive test**: Detects the issue on intentionally vulnerable contract patterns.
2. **Negative test**: Verifies zero warnings on idiomatically guarded, secure contract patterns.
3. **Boundary test**: Validates behavior on complex macros, early returns, and edge-case syntax.

```rust
#[test]
fn test_oracle_stale_heartbeat_positive() {
    let source = r#"
        pub fn liquidate(env: Env, oracle: Address) {
            let price = PriceOracleClient::new(&env, &oracle).get_price();
            execute_liquidation(price);
        }
    "#;
    let rule = OracleStaleHeartbeatRule::new();
    let violations = rule.check(source);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].rule_name, "oracle_stale_heartbeat");
}

#[test]
fn test_oracle_stale_heartbeat_negative_guarded() {
    let source = r#"
        pub fn liquidate(env: Env, oracle: Address) {
            let (price, timestamp) = PriceOracleClient::new(&env, &oracle).get_price_data();
            if env.ledger().timestamp() - timestamp > 300 {
                panic!("Stale oracle data");
            }
            execute_liquidation(price);
        }
    "#;
    let rule = OracleStaleHeartbeatRule::new();
    let violations = rule.check(source);
    assert!(violations.is_empty(), "Guarded price read must not produce false positive");
}
```

### Step 4: Documentation

Provide documentation in `docs/detectors/community/<rule_name>.md` detailing:
- Vulnerability explanation & real-world exploit scenario.
- Insecure contract sample.
- Secure contract sample.
- Remediation guidance.

---

## 5. Review & Acceptance Rubric

Community submissions are evaluated against four objective criteria:

```text
┌─────────────────┐     ┌──────────────────┐     ┌──────────────────┐
│  PR Submission  │ ──▶ │ AST Benchmarking │ ──▶ │ False-Positive   │
│  & Unit Proof   │     │ (<10ms Budget)   │     │ Corpus Audit (0) │
└─────────────────┘     └──────────────────┘     └──────────────────┘
                                                           │
                                                           ▼
┌─────────────────┐     ┌──────────────────┐     ┌──────────────────┐
│ Core Promotion  │ ◀── │ Registry Publish │ ◀── │ Maintainer Code  │
│ (Widespread)    │     │ (Community Tier) │     │ Review Approval  │
└─────────────────┘     └──────────────────┘     └──────────────────┘
```

1. **Accuracy & False-Positive Rate**:
   - The detector must have a verified **0% false-positive rate** across standard idiomatic Soroban SDK examples (`soroban-examples`).
   - Must not flag code using canonical guard constructs (`require_auth()`, `assert!`, `panic_with_error!`).

2. **AST Visitor Performance**:
   - Must execute within **< 10ms per source file** on standard developer hardware to maintain fast pre-commit and CI feedback loops.
   - Avoid quadratic-time AST traversals or repeated string parsing.

3. **Security & Defensive Framing**:
   - Adheres to standard defensive protocol quality assurance framing.
   - Findings must provide actionable remediation advice for contract maintainers.

4. **Code Quality**:
   - Passes `cargo fmt --all --check`.
   - Passes `cargo clippy -- -D warnings`.
   - Clean, idiomatic Rust code with zero unsafe blocks.

---

## 6. Enabling Community Rules

To use community-contributed rules in your local projects:

### Via Configuration (`.sanctify.toml`)

```toml
[rules]
# Enable standard core rules
default = true

# Enable curated community registry rules
[rules.community]
enable = [
    "oracle_stale_heartbeat",
    "unbounded_token_holder_loop",
    "missing_instance_ttl_bump"
]
```

### Via CLI Execution

```bash
sanctifier check --community-rule oracle_stale_heartbeat ./contracts/vault
```

---

## 7. Reporting Rule Issues & Feedback

If you encounter false positives, performance bottlenecks, or boundary edge cases with any community rule:
1. Open an issue on GitHub tagged `[community-rule] <rule-id>: <short description>`.
2. Provide a minimal reproducible contract snippet.
3. Our security working group reviews and coordinates rule patches within 48 hours.
