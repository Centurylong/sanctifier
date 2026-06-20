# Sanctifier ðŸ›¡ï¸

<p align="center">
  <img src="branding/logo.png" width="300" alt="Sanctifier Logo">
</p>

**Sanctifier** is a comprehensive security and formal verification suite built specifically for [Stellar Soroban](https://soroban.stellar.org/) smart contracts. In the high-stakes environment of DeFi and decentralized applications, "code is law" only holds true if the code is secure. Sanctifier ensures your contracts are not just compiled, but *sanctified*â€”rigorously tested, formally verified, and runtime-guarded against vulnerabilities.

## ðŸ“‚ Project Structure

```text
Sanctifier/
â”œâ”€â”€ contracts/          # Soroban smart contracts (examples & templates)
â”œâ”€â”€ frontend/           # Next.js Web Interface for the suite
â”œâ”€â”€ tooling/            # The core Rust analysis tools
â”‚   â”œâ”€â”€ sanctifier-cli  # CLI tool for developers
â”‚   â””â”€â”€ sanctifier-core # Static analysis logic
â”œâ”€â”€ scripts/            # Deployment and CI scripts
â””â”€â”€ docs/               # Documentation
```

## ðŸš€ Key Features

### 1. Static Sanctification (Static Analysis)
Sanctifier scans your Rust/Soroban code before deployment to detect:
*   **Authorization Gaps**: ensuring `require_auth` is present in all privileged functions.
*   **Storage Collisions**: analyzing `Instance`, `Persistent`, and `Temporary` storage keys.
*   **Resource Exhaustion**: estimating instruction counts to prevent OOG.

### 2. Runtime Guardians
A library of hook-based guards that you can integrate into your contracts:
*   Runtime invariant checks via `SanctifiedGuard`.
*   Step-by-step integration guide: [`docs/runtime-guards-integration.md`](docs/runtime-guards-integration.md)

### 3. Automated Deployment & Validation (NEW!)
Deploy runtime guard wrapper contracts to Soroban testnet with continuous validation:
*   **CLI Deployment**: One-command contract deployment with `sanctifier deploy`
*   **Bash Automation**: Production-ready scripts for testnet deployment
*   **CI/CD Integration**: GitHub Actions workflow for automated deployment and monitoring
*   **Continuous Validation**: Periodic health checks and execution metrics collection

## ðŸ“¦ Installation (CLI)

```bash
cargo install --path tooling/sanctifier-cli
```

## ðŸ›  Usage

### Analyze a Project
Run the analysis suite on your Soroban project:

```bash
sanctifier analyze ./contracts/my-token
```

#### Sample Output

When you run an analysis, Sanctifier displays security findings and recommendations:

```
âœ¨ Sanctifier: Valid Soroban project found at "./contracts/my-token"
ðŸ” Analyzing contract at "./contracts/my-token"...
âœ… Static analysis complete.

ðŸ›‘ Found potential Authentication Gaps!
   -> Function `transfer` is modifying state without require_auth()

ðŸ›‘ Found explicit Panics/Unwraps!
   -> Function `mint`: Using `unwrap` (Location: src/lib.rs:transfer)
   ðŸ’¡ Tip: Prefer returning Result or Error types for better contract safety.

ðŸ”¢ Found unchecked Arithmetic Operations!
   -> Function `compound_interest`: Unchecked `+` (src/lib.rs:compound_interest)
      ðŸ’¡ Use checked_add() or saturating_add() to prevent overflow.

âš ï¸  Found Ledger Size Warnings!
   LargeState approaches the ledger entry size limit!
      Estimated size: 68200 bytes (Limit: 64000 bytes)

ðŸ”„ Upgrade Pattern Analysis
   -> [missing_init] Contract has upgrade mechanism but no init function (src/lib.rs:42)
      ðŸ’¡ Add an init() function to set post-upgrade state safely.
```

For detailed explanations of each finding type and how to fix them, see [docs/getting-started.md](docs/getting-started.md#5-example-output--what-developers-see).

### Notify Webhooks on Scan Completion
Send scan completion notifications to one or more webhook endpoints:

```bash
sanctifier analyze ./contracts/my-token --webhook-url https://hooks.slack.com/services/XXX/YYY/ZZZ --webhook-url https://discord.com/api/webhooks/ID/TOKEN
```

### Verify Contract Invariants
Declare invariants with `#[sanctify::invariant(EXPR)]` and verify them:

```rust
// In your contract:
use sanctify_macros::invariant;

#[invariant(pure::supply_is_conserved_after_transfer(0, 0, 0))]
#[contractimpl]
impl Token { ... }
```

```bash
# Scan a contract or workspace for declared invariants and check them
sanctifier verify ./contracts/my-token

# CI mode — exit non-zero if any invariant is refuted
sanctifier verify ./contracts --strict

# Machine-readable output
sanctifier verify ./contracts --json

# Full symbolic proof via Kani (for function-call invariants)
cargo kani --package my-token
```

Invariants that reference pure functions (no `soroban_sdk::Env`) are dispatched
to the Z3 SMT backend. Complex expressions are reported as `KANI ↗` with a
reminder to run `cargo kani`. See `contracts/token-invariants` for a complete
example.

### Update Sanctifier
Check for and download the latest Sanctifier binary:

```bash
sanctifier update
```

### Generate a README Security Badge
Create an SVG badge and markdown snippet from a JSON scan report:

```bash
sanctifier analyze . --format json > sanctifier-report.json
sanctifier badge --report sanctifier-report.json --svg-output badges/sanctifier-security.svg --markdown-output badges/sanctifier-security.md
```


## Documentation

The full, cross-linked documentation set lives in [`docs/`](docs/README.md):

- **[Migration Guide](docs/migration.md)** — add Sanctifier to an existing Soroban repo (first scan → baseline → CI gate).
- **[CLI Reference](docs/cli.md)** — every command and flag, auto-generated from the parser and verified in CI.
- **[Configuration Reference](docs/configuration.md)** — every `.sanctify.toml` key, type, default, and precedence.
- **[FAQ & Troubleshooting](docs/faq.md)** — common questions plus an error → fix table.
- **[Glossary](docs/glossary.md)** — Soroban/Stellar security terms, anchored for deep-linking.

Start at the [documentation index](docs/README.md).


## Security Resources

Found a vulnerability in Sanctifier itself? Please do not open a public issue.
Use our [Security Policy](SECURITY.md) for the private advisory reporting
channel, disclosure timeline, scope, and safe-harbor terms.

Looking for Soroban security tools, audit reports, incident post-mortems, and learning materials? Check out our curated list:

**[Awesome Soroban Security](docs/awesome-soroban-security.md)** — a categorized collection of tools, audits, incidents, standards, and learning resources for building secure Soroban contracts.

## ðŸ¤ Contributing
We welcome contributions from the Stellar community! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

Every detector is covered by a golden `insta` snapshot so its findings can't regress unnoticed. If you add or change a detector, regenerate and review the snapshots — see [tooling/sanctifier-core/tests/README.md](tooling/sanctifier-core/tests/README.md).

## ðŸ”Ž Finding Codes
Unified finding codes (`S001`...`S016`) are documented in [docs/error-codes.md](docs/error-codes.md). For definitions of the underlying security terms, see the [Glossary](docs/glossary.md).

## ðŸ“„ License
MIT
