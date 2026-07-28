# Glossary of Soroban & Stellar Security Terms

Definitions for the Soroban/Stellar and security concepts used throughout the
Sanctifier docs. Every term has a **stable anchor** so findings, reports, and
other pages can deep-link to it — e.g. `glossary.md#require_auth` or
`glossary.md#oog-out-of-gas`.

This page is part of the [core documentation set](README.md). Finding codes
referenced below are listed in [Finding Codes](error-codes.md), and each has a
full write-up (vulnerable example + fix) in the
[Detector Catalog](detectors/README.md).

**Sections:** [Platform](#platform) · [Authorization](#authorization) ·
[Storage & ledger](#storage--ledger) · [Resources & cost](#resources--cost) ·
[Code-safety](#code-safety) · [Contract lifecycle](#contract-lifecycle) ·
[Verification](#verification) · [Tooling](#tooling)

---

## Platform

### Stellar
<a id="stellar"></a>
An open-source, public blockchain network for payments and asset issuance.
Soroban is Stellar's smart-contract platform. See [stellar.org](https://stellar.org/).

### Soroban
<a id="soroban"></a>
Stellar's smart-contract platform. Contracts are written in Rust, compiled to
[WASM](#wasm), and executed by the Soroban host. See
[soroban.stellar.org](https://soroban.stellar.org/).

### WASM
<a id="wasm"></a>
*WebAssembly* — the bytecode format Soroban contracts compile to (target
`wasm32-unknown-unknown`). Sanctifier analyzes **Rust source**, not compiled
WASM; problems that only surface when building for `wasm32` (e.g. a non-`no_std`
dependency) are build issues, not Sanctifier findings.

### Smart contract
<a id="smart-contract"></a>
Program deployed on-chain whose code deterministically governs state
transitions. On Soroban, a Rust module annotated with `#[contract]` /
`#[contractimpl]`.

### Host function / `Env`
<a id="host-function--env"></a>
Capabilities the Soroban host exposes to a contract (storage, crypto, auth,
cross-contract calls), accessed through the `soroban_sdk::Env` handle passed to
contract functions.

### `no_std`
<a id="no_std"></a>
A Rust crate that does not depend on the standard library. Soroban contracts are
`no_std`; a dependency that requires `std` will fail to build for `wasm32`.

---

## Authorization

### Authentication
<a id="authentication"></a>
Establishing *who* is making a call (which [address](#address)). Distinct from
[authorization](#authorization-term).

### Authorization
<a id="authorization-term"></a>
Deciding whether an authenticated caller is *allowed* to perform an action. In
Soroban this is enforced with [`require_auth`](#require_auth).

### `require_auth`
<a id="require_auth"></a>
Soroban host call (`Address::require_auth` / `require_auth_for_args`) that asserts
the given [address](#address) authorized the current invocation. Omitting it on a
state-mutating function is an **authorization gap** ([`S001`](error-codes.md)).

### Authorization gap (auth gap)
<a id="authorization-gap-auth-gap"></a>
A privileged or state-mutating function that changes state without calling
[`require_auth`](#require_auth), allowing unauthorized callers to act. Detected as
[`S001`](error-codes.md).

### Privileged / admin function
<a id="privileged--admin-function"></a>
A function whose effects (minting, upgrading, changing config) should be
restricted to a specific admin [address](#address). Must be guarded by
[`require_auth`](#require_auth).

### Address
<a id="address"></a>
Soroban `Address` — identifies an account or a contract. Used as the subject of
[`require_auth`](#require_auth) and as the holder in token operations.

### Hardcoded address
<a id="hardcoded-address"></a>
An admin address or secret literal embedded directly in source instead of being
configured/stored. Brittle and a security risk; detected as
[`S012`](error-codes.md).

---

## Storage & ledger

### Ledger
<a id="ledger"></a>
The replicated state of the network at a point in time. Contract data lives in
**ledger entries**.

### Ledger entry
<a id="ledger-entry"></a>
A single unit of stored on-chain state (a key/value record a contract reads and
writes). Subject to a maximum size.

### Ledger entry size limit
<a id="ledger-entry-size-limit"></a>
The maximum byte size of a single ledger entry. Sanctifier models this as
[`ledger_limit`](configuration.md#ledger_limit) (default `64000`) and flags state
approaching or exceeding it ([`S004`](error-codes.md)).

### Storage durability
<a id="storage-durability"></a>
Soroban classifies stored data by lifetime: [Instance](#instance-storage),
[Persistent](#persistent-storage), and [Temporary](#temporary-storage). Choosing
the wrong durability wastes rent or risks unexpected archival.

### Instance storage
<a id="instance-storage"></a>
Storage tied to the contract instance itself; loaded with the contract. Best for
small, always-needed config.

### Persistent storage
<a id="persistent-storage"></a>
Long-lived per-key storage that survives across invocations and must be kept
alive via [TTL bumps](#state-archival--ttl). Best for user balances and durable
records.

### Temporary storage
<a id="temporary-storage"></a>
Short-lived storage that can be archived/expired cheaply. Best for ephemeral data
(nonces, short-term caches) to reduce cost and [OOG](#oog-out-of-gas) risk.

### State archival / TTL
<a id="state-archival--ttl"></a>
Soroban reclaims entries whose *time-to-live* expires; contracts "bump" the TTL
to keep [persistent](#persistent-storage) data live. Forgetting to bump can make
data inaccessible.

### Storage key collision
<a id="storage-key-collision"></a>
Two logically distinct pieces of state mapping to the same storage key, so one
overwrites the other. Detected as [`S005`](error-codes.md).

---

## Resources & cost

### Resource metering / budget
<a id="resource-metering--budget"></a>
Soroban meters CPU instructions and memory per invocation against a budget.
Exceeding the budget aborts the call ([OOG](#oog-out-of-gas)).

### OOG (Out of Gas)
<a id="oog-out-of-gas"></a>
An invocation that exhausts its resource/CPU budget and aborts. Driven by
oversized state, unbounded loops, or expensive operations. Sanctifier's
ledger-size and resource heuristics ([`S004`](error-codes.md)) surface the common
causes.

### Instruction count
<a id="instruction-count"></a>
The metered number of CPU instructions an invocation executes; a primary
component of the resource budget.

### Footprint
<a id="footprint"></a>
The set of ledger entries an invocation declares it will read/write. An accurate,
minimal footprint keeps cost and risk down.

---

## Code-safety

### Panic
<a id="panic"></a>
An unrecoverable Rust abort (`panic!`, or via [`unwrap`/`expect`](#unwrap--expect)).
In a contract this traps the invocation. Detected as [`S002`](error-codes.md);
prefer returning a `Result`/contract error.

### `unwrap` / `expect`
<a id="unwrap--expect"></a>
`Option`/`Result` methods that [panic](#panic) on the absent/error case. Common
source of avoidable contract aborts.

### Unhandled `Result`
<a id="unhandled-result"></a>
A function call returning a `Result` whose value is ignored, so an error path is
silently dropped. Detected as [`S009`](error-codes.md).

### Arithmetic overflow / underflow
<a id="arithmetic-overflow--underflow"></a>
An integer operation that exceeds the type's range, wrapping or panicking.
Unchecked arithmetic in value/balance math is dangerous; detected as
[`S003`](error-codes.md).

### `checked_add` / `saturating_add`
<a id="checked_add--saturating_add"></a>
Safe arithmetic methods: `checked_*` returns `None` on overflow (handle
explicitly); `saturating_*` clamps to the type bound. Preferred over raw `+`/`-`/`*`
in contract math.

### Edge-amount validation
<a id="edge-amount-validation"></a>
Guards on token operations such as `amount > 0` and `from != to`. Missing these
allows no-op or self-transfer abuse; detected as [`S013`](error-codes.md).

### Reentrancy
<a id="reentrancy"></a>
A vulnerability where a contract is re-entered (often via a
[cross-contract call](#cross-contract-call)) before its first invocation finishes,
observing inconsistent intermediate state. Mitigated with checks-effects-interactions
ordering and runtime guards.

### `unsafe`
<a id="unsafe"></a>
Rust code that opts out of compiler safety guarantees. Rare and discouraged in
contracts; flagged generically as [`S006`](error-codes.md) and a common target for
a [custom rule](configuration.md#custom_rules).

### Dead code
<a id="dead-code"></a>
Unreachable code or always-true/false guards (e.g. detectable via constant
folding). Detected as [`S015`](error-codes.md); often signals a logic mistake.

### Error code collision
<a id="error-code-collision"></a>
Duplicate or inconsistent discriminants in a `#[contracterror]` enum, so distinct
errors are indistinguishable on-chain. Detected as [`S016`](error-codes.md).

### Event
<a id="event"></a>
A log record a contract publishes (topics + data) for off-chain consumers.
Inconsistent topic counts or wasteful patterns are detected as
[`S008`](error-codes.md).

---

## Contract lifecycle

### Contract upgrade
<a id="contract-upgrade"></a>
Replacing a deployed contract's WASM (e.g. via
[`update_current_contract_wasm`](#update_current_contract_wasm)). Powerful and
risky; upgrade/admin mechanisms are analyzed for [`S010`](error-codes.md).

### `update_current_contract_wasm`
<a id="update_current_contract_wasm"></a>
The Soroban host call that swaps a contract's code. Must be tightly
[authorized](#require_auth) and paired with safe post-upgrade
[initialization](#initialization).

### Initialization
<a id="initialization"></a>
A one-time `init` step that sets required state (admin, config). An upgrade
mechanism without an init path is an [upgrade risk](#contract-upgrade)
([`S010`](error-codes.md)).

### Token contract
<a id="token-contract"></a>
A contract implementing fungible-token semantics (balances, transfers, mint,
burn). The subject of Sanctifier's built-in invariants.

### Mint / Burn
<a id="mint--burn"></a>
Creating (`mint`) or destroying (`burn`) token units. Must be authorized and
conserve [supply](#supply_conserved); unauthorized mint is a classic exploit
(see [`no_unauthorized_mint`](#no_unauthorized_mint)).

### Cross-contract call
<a id="cross-contract-call"></a>
One contract invoking another (`env.invoke_contract`). Edges are extracted by
[`sanctifier callgraph`](cli.md#sanctifier-callgraph); a vector for
[reentrancy](#reentrancy).

### Call graph
<a id="call-graph"></a>
A directed graph of cross-contract calls, emitted as Graphviz DOT by
[`callgraph`](cli.md#sanctifier-callgraph), used to reason about trust boundaries.

---

## Verification

### Invariant
<a id="invariant"></a>
A property that must always hold (e.g. total supply is conserved). Declared with
`#[sanctify::invariant(EXPR)]` and checked by
[`verify`](cli.md#sanctifier-verify); a refuted invariant is
[`S011`](error-codes.md).

### Formal verification
<a id="formal-verification"></a>
Mathematically proving (or refuting) that code satisfies a specification, rather
than testing samples. Sanctifier uses [SMT](#smt) and optionally
[Kani](#kani).

### SMT
<a id="smt"></a>
*Satisfiability Modulo Theories* — the technique behind automated proof of
arithmetic/logic properties. Sanctifier dispatches pure-function
[invariants](#invariant) to the [Z3](#z3) SMT solver.

### Z3
<a id="z3"></a>
The SMT solver Sanctifier links for [`verify`](cli.md#sanctifier-verify) and
[`prove`](cli.md#sanctifier-prove). Requires the Z3 C headers at build time — see
the [FAQ](faq.md#z3--dbus-build-errors).

### Kani
<a id="kani"></a>
A Rust model checker for deeper, function-call-level proofs. Complex invariants
are reported as `KANI ↗` with a reminder to run `cargo kani`.

### Proof certificate
<a id="proof-certificate"></a>
An on-disk artifact recording the result of an SMT proof, written by
[`prove`](cli.md#sanctifier-prove) (skip with `--no-save`).

### `supply_conserved`
<a id="supply_conserved"></a>
Built-in token invariant: total supply is unchanged by transfers. Provable via
`sanctifier prove --invariant supply_conserved`.

### `balance_non_negative`
<a id="balance_non_negative"></a>
Built-in token invariant: no account balance can become negative.

### `no_unauthorized_mint`
<a id="no_unauthorized_mint"></a>
Built-in token invariant: tokens cannot be minted without proper
[authorization](#require_auth).

---

## Tooling

### Static analysis
<a id="static-analysis"></a>
Analyzing source code without executing it. Sanctifier's `analyze` parses Rust to
find security-relevant patterns.

### Finding code
<a id="finding-code"></a>
A stable identifier (`S001`…`S016`) for a class of finding, shared across CLI and
JSON output. See [Finding Codes](error-codes.md).

### Severity
<a id="severity"></a>
The seriousness of a finding. Sanctifier groups results into **critical** and
**high** for exit-code purposes; [custom rules](configuration.md#custom_rules)
accept `info`, `warning`, or `error`.

### False positive
<a id="false-positive"></a>
A reported finding that is not actually a problem. Static analysis is conservative
and produces some; handling strategies are in the
[FAQ](faq.md#sanctifier-flagged-something-that-is-actually-safe-how-do-i-handle-a-false-positive).

### False negative
<a id="false-negative"></a>
A real problem the analyzer did **not** report. Why Sanctifier complements, but
does not replace, audits and tests.

### Golden snapshot
<a id="golden-snapshot"></a>
A reviewed reference output (`insta` snapshot) each detector is tested against, so
its findings cannot change unnoticed. See
[tooling/sanctifier-core/tests](../tooling/sanctifier-core/tests).

### Custom rule
<a id="custom-rule"></a>
A user-defined regex check configured under
[`[[custom_rules]]`](configuration.md#custom_rules); matches are reported as
[`S007`](error-codes.md).

---

## See also

- [CLI Reference](cli.md) · [Configuration Reference](configuration.md) ·
  [Migration Guide](migration.md) · [FAQ & Troubleshooting](faq.md) ·
  [Finding Codes](error-codes.md)
