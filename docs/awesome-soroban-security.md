# Awesome Soroban Security [![Awesome](https://awesome.re/badge.svg)](https://awesome.re)

> A curated list of security resources, tools, audits, incident reports, and standards for [Stellar Soroban](https://soroban.stellar.org/) smart contract development.

Maintained by the [Sanctifier](https://github.com/Ardecrownn/sanctifier) project — a security and formal-verification suite for Soroban contracts.

---

## Contents

- [Tools](#tools)
  - [Static Analysis](#static-analysis)
  - [Fuzzing & Property-Based Testing](#fuzzing--property-based-testing)
  - [Formal Verification](#formal-verification)
  - [Runtime Guards](#runtime-guards)
- [Audits & Audit Firms](#audits--audit-firms)
- [Incident Reports & Post-Mortems](#incident-reports--post-mortems)
- [Standards & Best Practices](#standards--best-practices)
- [Learning Resources](#learning-resources)
  - [Official Documentation](#official-documentation)
  - [Tutorials & Guides](#tutorials--guides)
  - [Videos & Talks](#videos--talks)
- [Communities](#communities)
- [Contributing](#contributing)

---

## Tools

### Static Analysis

- [Sanctifier](https://github.com/Ardecrownn/sanctifier) - Comprehensive static analysis CLI and suite for Soroban contracts. Detects auth gaps, storage collisions, arithmetic overflows, resource exhaustion, and more.
- [Cargo Clippy](https://doc.rust-lang.org/clippy/) - The official Rust linter. Catches common mistakes and enforces idiomatic Rust patterns relevant to Soroban contract code.
- [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) - Audits `Cargo.lock` for crates with known security vulnerabilities reported to the [RustSec Advisory Database](https://rustsec.org/).
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny) - Lint your project's dependency graph for banned crates, duplicate versions, license issues, and known advisories.
- [semgrep](https://semgrep.dev/) - Fast, open-source static analysis tool. Can be used with custom Rust rules to detect Soroban-specific anti-patterns.

### Fuzzing & Property-Based Testing

- [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) - A command-line wrapper for libFuzzer for Rust. Useful for fuzz-testing Soroban contract logic off-chain.
- [Bolero](https://camshaft.github.io/bolero/) - A property testing and fuzzing framework for Rust, supporting multiple backends (libFuzzer, AFL, honggfuzz).
- [proptest](https://github.com/proptest-rs/proptest) - Hypothesis-style property-based testing for Rust. Good for generating adversarial inputs against contract logic.
- [Stellar Soroban Test Utilities](https://docs.rs/soroban-sdk/latest/soroban_sdk/testutils/index.html) - The official `soroban-sdk` test utilities, including mock environments for auth and ledger state.

### Formal Verification

- [Kani Rust Verifier](https://github.com/model-checking/kani) - A bit-precise model checker for Rust. Can prove safety properties of Soroban contract logic with bounded verification. See also the [Sanctifier Kani integration guide](../docs/kani-integration.md).
- [MIRAI](https://github.com/endorlabs/MIRAI) - An abstract interpreter for Rust's MIR, capable of detecting panics, type errors, and security-relevant invariants statically.
- [Prusti](https://github.com/viperproject/prusti-dev) - A static verifier for Rust based on the Viper verification infrastructure. Supports pre/post-conditions and loop invariants.

### Runtime Guards

- [Sanctifier Runtime Guards](../docs/runtime-guards-integration.md) - Drop-in `SanctifiedGuard` hooks for invariant checks, circuit breakers, and emergency shutdown patterns inside Soroban contracts.
- [soroban-sdk Authorization Helpers](https://docs.rs/soroban-sdk/latest/soroban_sdk/auth/index.html) - Official SDK helpers for `require_auth`, `require_auth_for_args`, and address-level authorization enforcement.

---

## Audits & Audit Firms

> Public audit reports and firms that have demonstrated Soroban / Stellar smart contract expertise.

| Firm | Notable Work | Report Link |
|------|-------------|-------------|
| [OtterSec](https://osec.io/) | Multiple Soroban DeFi audits | [Reports Index](https://github.com/otter-sec/reports) |
| [Trail of Bits](https://www.trailofbits.com/) | Stellar core, cryptographic tooling | [Publications](https://github.com/trailofbits/publications) |
| [Certora](https://www.certora.com/) | Formal verification of DeFi protocols (EVM + expanding to Soroban) | [Case Studies](https://www.certora.com/case-studies) |
| [Halborn](https://www.halborn.com/) | Stellar ecosystem projects | [Reports](https://www.halborn.com/reports) |
| [Cure53](https://cure53.de/) | Cryptographic and protocol-level reviews | [Publications](https://cure53.de/advisories.php) |
| [Bishop Fox](https://bishopfox.com/) | Web3 and blockchain security assessments | [Research](https://bishopfox.com/blog) |

> **Note:** If you are looking for a Soroban-specialized audit, check the [Stellar Community Fund](https://communityfund.stellar.org/) for currently funded security projects.

---

## Incident Reports & Post-Mortems

> Documented incidents from Stellar and broader Web3 ecosystems with lessons applicable to Soroban development.

- [Stellar Network Inflation Bug (2019)](https://www.stellar.org/blog/protocol-upgrade-stellar-inflation-bug) - A protocol-level bug that allowed unauthorized XLM inflation; illustrates the importance of arithmetic and state invariant checks.
- [Soroban Security Considerations — Stellar Dev Blog](https://stellar.org/blog) - Ongoing developer blog covering security topics, protocol upgrades, and lessons from testnet.
- [SWC Registry (Ethereum)](https://swcregistry.io/) - While EVM-focused, the Smart Contract Weakness Classification registry documents vulnerability classes (reentrancy, tx.origin misuse, etc.) whose conceptual analogues exist in Soroban.
- [Rekt News](https://rekt.news/) - Aggregated DeFi incident post-mortems. Useful for understanding real-world attack vectors and their financial impact, even when Soroban-specific incidents are limited.
- [DeFiHackLabs](https://github.com/SunWeb3Sec/DeFiHackLabs) - A repository of PoC exploits for past DeFi incidents; useful for understanding attack patterns to guard against in Soroban contract design.

---

## Standards & Best Practices

- [Soroban Security Best Practices (Official)](https://developers.stellar.org/docs/build/smart-contracts/guides/security) - Stellar Developer documentation covering auth, storage, error handling, and upgrade patterns.
- [Stellar Developer Security Guidelines](https://developers.stellar.org/docs/learn/encyclopedia/security) - Encyclopedia entry on Stellar/Soroban security fundamentals.
- [Rust Secure Code Working Group Guidelines](https://anssi-fr.github.io/rust-guide/) - ANSSI's comprehensive guide for writing secure Rust, directly applicable to Soroban contract code.
- [OWASP Smart Contract Top 10](https://owasp.org/www-project-smart-contract-top-10/) - The top 10 smart contract vulnerabilities ranked by frequency and impact; maps well to Soroban concerns.
- [Sanctifier Finding Codes](../docs/error-codes.md) - Unified finding codes (`S001`–`S007`) used by Sanctifier's static analysis engine, with remediation guidance.
- [CWE — Common Weakness Enumeration](https://cwe.mitre.org/) - MITRE's comprehensive weakness taxonomy. Many Soroban vulnerability classes map to standard CWE entries.
- [SEI CERT Rust Coding Standard](https://wiki.sei.cmu.edu/confluence/display/rust/) - Secure coding rules for Rust from Carnegie Mellon's Software Engineering Institute.

---

## Learning Resources

### Official Documentation

- [Soroban Developer Documentation](https://developers.stellar.org/docs/build/smart-contracts) - The primary reference for writing, deploying, and testing Soroban contracts.
- [soroban-sdk API Docs](https://docs.rs/soroban-sdk/latest/soroban_sdk/) - Full API reference for the Soroban Rust SDK.
- [Stellar Developers Blog](https://stellar.org/blog/developers) - Announcements, protocol updates, and deep dives from the Stellar Development Foundation.
- [Stellar Protocol Upgrade Notes](https://github.com/stellar/stellar-protocol/tree/master/core) - CAPs (Core Advancement Proposals) documenting protocol-level changes and security considerations.

### Tutorials & Guides

- [Soroban Examples](https://github.com/stellar/soroban-examples) - Official example contracts demonstrating patterns for tokens, DEXes, multisig, and more.
- [Sanctifier Getting Started Guide](../docs/getting-started.md) - Step-by-step guide to running Sanctifier's analysis suite on your Soroban project.
- [Sanctifier Kani Integration Guide](../docs/kani-integration.md) - How to set up and run formal verification proofs on Soroban contract logic using Kani.
- [Sanctifier Runtime Guards Integration](../docs/runtime-guards-integration.md) - Adding invariant checks and circuit breakers to Soroban contracts.
- [Soroban Smart Contract Security Checklist](https://developers.stellar.org/docs/build/smart-contracts/guides/security) - A developer checklist for pre-deployment security review.
- [Rust Book — Fearless Concurrency & Ownership](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html) - Foundation concepts that underpin Rust's memory-safety guarantees, critical for secure contract code.

### Videos & Talks

- [Sanctifier Formal Verification Video Series](../docs/formal-verification-video-series.md) - Recorded walkthroughs of formal verification workflows using Kani with Soroban contracts.
- [Stellar Developer Office Hours](https://www.youtube.com/@StellarDevelopmentFoundation) - Regular video sessions covering Soroban development topics, including security Q&A.
- [Soroban Deep Dives — SDF YouTube](https://www.youtube.com/@StellarDevelopmentFoundation) - Recorded developer workshops and protocol explanations from the Stellar Development Foundation.

---

## Communities

- [Stellar Developers Discord](https://discord.gg/stellardev) - The main developer community for Stellar and Soroban, with dedicated channels for security and smart contracts.
- [Stellar Stack Exchange](https://stellar.stackexchange.com/) - Q&A community for Stellar developers; good for specific security implementation questions.
- [Stellar Community Forum](https://community.stellar.org/) - Discussion forum for ecosystem projects, governance, and developer topics.
- [Sanctifier GitHub Discussions](https://github.com/Ardecrownn/sanctifier/discussions) - Ask questions, share findings, and discuss Soroban security topics with Sanctifier contributors.
- [RustSec Advisory Database](https://rustsec.org/) - Database of security advisories for Rust crates; subscribe to stay current on dependency vulnerabilities.

---

## Contributing

Contributions are very welcome. Please follow these guidelines to keep the list useful and high quality.

### What belongs here

A resource is a good fit if it:

- Is **directly relevant** to Soroban/Stellar smart contract security, or to securing Rust code that runs in a resource-constrained blockchain environment.
- Is **publicly accessible** (not behind a paywall or private login).
- Is **actively maintained** or is a notable historical reference (clearly labeled as such).
- Adds value not already covered by an existing entry.

### What does not belong here

- Generic blockchain security content with no Soroban/Rust applicability.
- Promotional content for commercial services without a demonstrated public benefit (e.g., a free tier, open audit reports, or open-source tooling).
- Duplicate links or near-duplicates of existing entries.

### How to add a resource

1. Fork this repository.
2. Create a branch: `git checkout -b add/<short-description>`.
3. Add your resource to the appropriate section in `docs/awesome-soroban-security.md`. Follow the existing link format:
   ```markdown
   - [Resource Name](https://link) - One-sentence description of what it is and why it is useful for Soroban security.
   ```
4. For audit firms or incident reports, use the table format already established in those sections.
5. Open a pull request with the title `[Awesome] Add: <Resource Name>` and a brief description of why the resource is valuable.
6. A maintainer will review and merge within a few days.

### Style guide

- Use sentence case for descriptions (capitalize the first word and proper nouns only).
- Keep descriptions to one or two sentences — enough to explain the value without being a full review.
- Verify that links are live before submitting.
- Prefer official or primary sources over mirrors or aggregators.
- When adding an audit firm, include at least one public report or evidence of Soroban/Stellar work.

### Reporting broken links

Open a GitHub issue with the title `[Awesome] Broken link: <Resource Name>` and include the section where the link appears.

---

<p align="center">
  Maintained with ❤️ by the <a href="https://github.com/Ardecrownn/sanctifier">Sanctifier</a> project · <a href="../CONTRIBUTING.md">General Contributing Guide</a>
</p>