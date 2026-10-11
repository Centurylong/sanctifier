# Sanctifier Adopters Showcase & Real Findings Gallery

This document showcases open-source and ecosystem projects using **Sanctifier** to audit, formally verify, and secure their Stellar/Soroban smart contracts, alongside a curated gallery of real, responsibly-disclosed findings discovered across codebases.

---

## ðŸ† Featured Adopters

| Project | Ecosystem Focus | Contract Scope | CI Integration | Verification Level |
| :--- | :--- | :--- | :--- | :--- |
| **StepFi Protocol** | Decentralized Credit Lines | `StepFi-Contracts` (creditline, pool, escrow) | GitHub Actions (`sanctifier check`) | Strict (`sep41`, `reentrancy`, `storage`) |
| **Sorobill Protocol** | Web3 Subscriptions & Recurring Invoicing | `Sorobill-Contract` (billing, ledger TTL) | CI Workflow + CLI Gate | Production (`auth-guards`, `ttl-bump`) |
| **SafeTrust Escrow** | Trustless P2P Escrow & Milestones | `contract-SafeTrust` (multisig, arbitration) | Pre-commit Hook + CI | High (`state-transition-soundness`) |
| **Chioma Housing** | Real-World Asset (RWA) Tokenization | `chioma/contracts` (property registry, lease) | GitHub Actions Matrix | Recommended (`overflow`, `instance-ttl`) |

---

## ðŸ” Responsibly-Disclosed Findings Gallery

Below is a gallery of real vulnerability patterns identified and mitigated by Sanctifier detectors in production and testnet environments.

### 1. SEP-41 Token Allowance Approval Race
* **Detector**: `sep41-approval-race` (Rule SEC-012)
* **Vulnerability Class**: Front-running allowance race condition.
* **Finding Summary**: Contracts executing token approvals without resetting allowance to zero first allowed spenders to execute an unapproved transfer before the new allowance transaction was committed.
* **Remediation**: Enforced atomic zero-first reset or `increase_allowance` / `decrease_allowance` delta functions.

### 2. Storage Key Collision Across Differing Value Types
* **Detector**: `storage-key-type-integrity` (Rule DAT-004)
* **Vulnerability Class**: Instance storage data desynchronization.
* **Finding Summary**: Reusing symbol-based keys (`DataKey::Admin`) with mismatched serialization types in different entrypoints caused deserialization panic during state reads.
* **Remediation**: Consolidated all storage keys behind typed enum discriminants with explicit serialization bounds.

### 3. Missing Ledger TTL Bump on Long-Lived Subscriptions
* **Detector**: `soroban-ttl-expiration` (Rule TTL-001)
* **Vulnerability Class**: Contract state expiration denial of service.
* **Finding Summary**: Contract state entries in Persistent storage lacked scheduled `extend_ttl` operations, risking automatic archival after the threshold period.
* **Remediation**: Added automated keeper bump hooks and threshold checks on user interactions.

### 4. Cross-Contract Reentrancy via External Invocations
* **Detector**: `reentrancy-effect-order` (Rule SEC-001)
* **Vulnerability Class**: Checks-Effects-Interactions violation.
* **Finding Summary**: Invoking `env.invoke_contract()` to disburse fees prior to decrementing internal balance mapping opened an exploitation path.
* **Remediation**: Reordered state mutations strictly before any external contract invocation (`CEI` pattern).

---

## ðŸ¤ Submitting Your Project

Are you using Sanctifier in your Stellar/Soroban project? We would love to feature your codebase!

1. Open a Pull Request adding your project details to the table above.
2. If you would like to contribute a sanitized, responsibly-disclosed finding to help the ecosystem, submit an entry to the Findings Gallery.
3. Include links to your public repository and CI verification workflow.