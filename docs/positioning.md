# Positioning: where Sanctifier fits (and where it doesn't)

> Tracking issue: **#772** — Documentation & Learning epic.

This page states, honestly, what Sanctifier is good for, what it is **not** a
substitute for, and how it sits alongside a manual audit and the other tools you
might already run. The goal is to help you make a sound adoption decision — and
to help grant reviewers judge the project on accurate claims rather than hype.

If you take one thing from this page: **Sanctifier is a fast, automated safety
net that catches known, machine-detectable classes of bug early and on every
commit. It raises your floor. It does not replace a human security audit, which
raises your ceiling.**

## What Sanctifier is

A security and formal-verification suite built specifically for
[Stellar Soroban](https://soroban.stellar.org/) (Rust → Wasm) contracts. It has
three layers:

1. **Static analysis** — a `syn`-based detector set that scans Rust source for
   known bug classes (missing `require_auth`, panics/`unwrap`, unchecked
   arithmetic, storage-TTL omissions, view functions that write state, and more;
   see the [Finding Codes](error-codes.md)). Runs in milliseconds, in CI, on
   every commit.
2. **Formal verification** — invariant checks and SMT/[Kani](kani-integration.md)
   integration for properties you specify, so certain guarantees are *proven*
   over all inputs rather than sampled by tests.
3. **Runtime guards** — hook-based invariant checks
   ([`runtime-guards-integration.md`](runtime-guards-integration.md)) that fail
   closed on-chain if an invariant is violated at execution time.

## What Sanctifier is **not**

- **Not an audit.** An audit is an expert human (or team) reasoning about your
  *specific* business logic, economic model, and threat model. Sanctifier has no
  understanding of what your protocol is *supposed* to do, so it cannot tell you
  that your fee math is exploitable, your oracle can be manipulated, or your
  governance timelock is too short. Those are the bugs that actually drain
  protocols, and they are found by people, not pattern-matchers.
- **Not a proof of correctness** (except for the specific invariants you write
  and verify). A clean static-analysis run means "none of the *known,
  encoded* bug patterns matched" — not "this contract is safe".
- **Not a guarantee against false positives or false negatives.** Static
  analysis trades completeness for speed. It will occasionally flag safe code
  (tune with [`.sanctify.toml`](configuration.md) or inline
  `// sanctifier:ignore[CODE]`) and will silently miss bugs outside its detector
  set.
- **Not a replacement for tests, fuzzing, or `clippy`.** It complements them.

## The security stack, and where each layer earns its keep

Think of contract security as layers, cheapest and shallowest first:

| Layer | Cost / speed | Catches | Misses |
|-------|--------------|---------|--------|
| `cargo clippy` + `cargo test` | free, seconds | general Rust mistakes, your own asserted behaviour | Soroban-specific & security-specific classes |
| **Sanctifier static analysis** | free, seconds, every commit | known Soroban security anti-patterns (auth gaps, panics, TTL, unchecked math, view-writes…) | novel logic bugs, economic exploits |
| **Sanctifier formal verification** | minutes, on the invariants you write | *proven* violations of specified invariants over all inputs | properties you didn't specify |
| Fuzzing / property testing | minutes–hours | edge-case inputs that break assertions | anything the harness doesn't explore |
| **Manual audit** | weeks, expensive, pre-mainnet | business-logic, economic, and design flaws | regressions after the audit ends |
| **Runtime guards** | on-chain gas cost | invariant violations *in production* | bugs outside the guarded invariants |

The layers are additive. Sanctifier's job is to make the cheap layers as strong
as possible so that expensive human review time is spent on the hard,
context-dependent problems only humans can reason about — not on catching a
missing `require_auth` that a linter should have caught on commit #1.

## Sanctifier and a manual audit are complementary, not competing

| | Manual audit | Sanctifier |
|---|---|---|
| **Runs** | once (or per major release) | on every commit, forever |
| **Understands business logic** | yes | no |
| **Finds novel/economic exploits** | yes | no |
| **Cost** | high (expert time) | ~free (automated) |
| **Speed** | weeks | seconds |
| **Coverage** | deep, contextual, finite in time | shallow, mechanical, continuous |
| **Regression protection after it runs** | none | catches reintroduced known-bad patterns forever |

The strongest workflow uses both: run Sanctifier from day one to keep the
codebase clean of known anti-patterns, so that when you pay for an audit the
auditors spend their time on your actual design instead of low-hanging fruit —
and after the audit, Sanctifier keeps guarding against regressions the auditors
will never see.

## Sanctifier vs other tooling

- **vs `clippy`** — `clippy` is a general Rust linter with no notion of Soroban
  storage, `require_auth`, TTLs, or DeFi threat models. Run both; they don't
  overlap much.
- **vs EVM analyzers (Slither, Aderyn)** — different platform (Solidity/EVM), so
  no line-for-line comparison is possible. We *do* maintain a class-level
  differential study of where coverage overlaps — see
  [Differential testing vs Slither/Aderyn](differential-testing.md).
- **vs other Soroban analyzers** — see the companion study
  [Differential testing vs other Soroban linters](differential-soroban-linters.md)
  for where Sanctifier and Soroban-native tools agree, disagree, and leave gaps.
- **vs auditing firms** — see the section above: complementary, not competing.

## Honest limitations (read before you rely on it)

- Detector coverage is **finite and enumerable** — see [Finding Codes](error-codes.md).
  If a bug class isn't listed there, Sanctifier does not look for it.
- Static analysis works on **source**; it cannot reason about deployment
  configuration, off-chain components, front-ends, or key management.
- Formal-verification guarantees are only as good as the invariants you write.
  An unspecified property is an unverified property.
- A green Sanctifier run is a **necessary-but-not-sufficient** signal for
  mainnet. Treat it as "the floor is clean", never as "we are safe to ship".

## When to reach for what

- **Writing code / on every commit:** static analysis (fast feedback, CI gate).
- **Before mainnet / for high-value contracts:** formal verification of your
  core invariants **and** a manual audit.
- **In production:** runtime guards for the invariants that must never break.
- **Choosing between Sanctifier and an audit:** you don't. Use both.

## See also

- [Migration Guide](migration.md) — get Sanctifier running in CI.
- [Finding Codes](error-codes.md) — exactly what the static analyzer looks for.
- [FAQ & Troubleshooting](faq.md) — including false-positive handling.
- [Differential testing vs Slither/Aderyn](differential-testing.md) and
  [vs other Soroban linters](differential-soroban-linters.md) — coverage
  comparisons.
- [Awesome Soroban Security](awesome-soroban-security.md) — external tools,
  audits, and learning resources.
