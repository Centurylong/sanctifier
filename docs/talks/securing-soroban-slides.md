---
marp: true
title: Securing Soroban Smart Contracts with Sanctifier
description: Conference talk / workshop slide deck. Renders with Marp or any Markdown-slides tool that splits on `---`.
paginate: true
theme: default
---

<!--
Securing Soroban Smart Contracts with Sanctifier
================================================
Reusable slide deck (issue #777).

HOW TO PRESENT
- Slides are plain Markdown separated by `---`. Any of these render them:
    * Marp:      `marp docs/talks/securing-soroban-slides.md --html -o slides.html`
    * reveal-md: `reveal-md docs/talks/securing-soroban-slides.md`
    * VS Code:   "Marp for VS Code" extension → open preview
- Speaker notes for each slide live in HTML comments like this one.
- The live-demo beats are scripted separately in `securing-soroban-demo-script.md`;
  the "DEMO" slides below are cue cards that tell you when to switch to a terminal.
- Timing: the deck is built for a 40-minute conference talk. The 90-minute
  workshop reuses the same slides and expands the DEMO beats into hands-on labs
  (see the workshop guide in `README.md`).
-->

# Securing Soroban Smart Contracts

### Static analysis + formal verification with **Sanctifier**

<br>

*A talk & hands-on workshop for Soroban / Stellar developers*

<!--
Speaker note: Open by asking the room two questions by show of hands:
1. Who has written a Soroban contract?
2. Who has had one audited?
The gap between those two hands is the reason this talk exists. Set expectations:
this is practical — we run a real tool on a real vulnerable contract and fix it live.
-->

---

## Who this is for

- You write **Soroban** smart contracts in Rust
- You want defects caught **before** mainnet, not by an attacker
- You want a repeatable **CI gate**, not a one-off audit

<br>

**You'll leave able to:** run Sanctifier on a contract, read its findings, fix
the top classes of bugs, and wire it into CI.

<!--
Speaker note: Emphasize "repeatable". Audits are point-in-time; code changes daily.
The goal is to make security a property of the pipeline, not an event.
-->

---

## Why Soroban security is different

Soroban isn't the EVM. The footguns are Soroban's own:

- **Authorization** — `require_auth` is explicit; forget it and anyone calls you
- **Storage & TTL** — persistent/instance entries expire; archival ≠ deletion
- **Resource limits** — unbounded loops over user input hit the ledger's metering
- **Panics** — a panic aborts the whole transaction; `unwrap` is a liability
- **Arithmetic** — `i128` wraps in release unless you use checked math

<br>

Generic Rust linters miss all five. You need Soroban-aware analysis.

<!--
Speaker note: This is the "why not just clippy?" slide. Clippy knows Rust; it does
not know that a state-mutating #[contractimpl] method without require_auth is a
security hole. Domain awareness is the whole point.
-->

---

## What Sanctifier is

A security + formal-verification suite for Soroban, in three layers:

1. **Static detectors** — 13 Soroban-aware rules (auth, panics, arithmetic,
   storage TTL, DoS, hygiene…) with stable finding codes `S001…S017`
2. **Formal verification** — Kani proof harnesses + an SMT (Z3) backend for
   invariants that tests can't exhaust
3. **CI-native tooling** — baselines, diffs, badges, attestations, SARIF/JSON

<br>

Open source. Runs on source **and** on compiled WASM (source-optional mode).

<!--
Speaker note: Don't over-index on the formal-verification layer for a general
audience — mention it's there, show detectors live. The FV deep-dive is its own
workshop track. "Runs on compiled WASM" lands well with auditors who receive
artifacts, not repos.
-->

---

## The detector catalog at a glance

| Code | Catches |
|---|---|
| `S001` | Missing authentication on a state-mutating entrypoint |
| `S002` / `SANCT_UNWRAP` | `panic!` / `unwrap` / `expect` that aborts a tx |
| `S003` | Unchecked arithmetic (overflow / underflow) |
| `S004` / `S006` | Ledger entry size / missing TTL extension |
| `S009` | A `Result` silently dropped |
| `S013` | `transfer`/`mint`/`burn` missing `amount>0` / `from!=to` |
| `SANCT_ARG_DOS` | `Vec`/`Map` argument iterated with no length cap |

<br>

Full catalog with a vulnerable example + fix per detector:
`docs/detectors/README.md`

<!--
Speaker note: You don't read this table aloud. Point at the three rows we're about
to trigger live — S001 (auth), S003 (overflow), SANCT_UNWRAP — and move on.
-->

---

## DEMO 1 — Meet the patient

`contracts/token-with-bugs` — a deliberately broken token.

Three planted bugs we'll find and fix:

- `transfer` — **no `from.require_auth()`** → anyone drains any account
- `mint` — **`current + amount`** with no overflow check → balance wraps
- `balance` — **`unwrap_or(0)`** hides missing/archived storage

<br>

➡️ *Switch to terminal — demo script beat 1.*

<!--
Speaker note: Show the source first (30s), then run the tool. Seeing the bug in
Rust before the tool names it builds trust that the tool isn't hand-waving.
-->

---

## DEMO 2 — Run the analyzer

```bash
sanctifier analyze ./contracts/token-with-bugs
```

Read the output top-down:

- **Summary** — counts by severity; triage critical/high first
- **Findings** — each has a **code**, a location, and a message
- Every code links to a detector page: *what it catches → the fix*

<br>

➡️ *Switch to terminal — demo script beat 2 & 3.*

<!--
Speaker note: Resist the urge to fix everything. Pick the auth finding (S001) and
follow it all the way to green, then batch the rest. Narrate the finding code out
loud so the audience connects code → catalog page.
-->

---

## The fix, one class at a time

```rust
// S001 — authorize the debited party
pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
    from.require_auth();                     // ← the fix
    // S013 — reject nonsense transfers
    assert!(amount > 0 && from != to);
    // S003 — checked arithmetic, no silent wrap
    let from_balance = Self::balance(e.clone(), from.clone());
    let new_from = from_balance.checked_sub(amount).expect("underflow");
    ...
}
```

<br>

➡️ *Re-run `analyze` — watch the count drop. Demo script beat 4.*

<!--
Speaker note: The "watch the count drop" moment is the emotional core of the talk.
Let it land. Re-running after each fix is more convincing than one big diff.
-->

---

## Make it a gate, not a chore

Security that isn't enforced decays. Wire Sanctifier into CI:

```bash
sanctifier baseline ./contracts/token-with-bugs   # snapshot known state
sanctifier diff --baseline .sanctify/baseline.json # fail on NEW findings
```

- **Baseline** accepts today's reality; **diff** blocks regressions
- Emits **SARIF** for GitHub code scanning, **JSON** for your own tooling
- A **badge** + **attestation** make the guarantee visible & verifiable

<!--
Speaker note: The baseline/diff pattern is how teams adopt a linter on a large
existing codebase without a "fix 400 findings" flag day. Meet teams where they are.
-->

---

## Beyond linting — formal verification

Detectors find known bug *shapes*. Some properties need proof:

- "`total_supply` always equals the sum of balances"
- "no `transfer` can create tokens"

<br>

Sanctifier pairs with **Kani** to model-check pure logic, and uses **Z3** for
invariant checking — exhaustive over inputs, not just the ones you tested.

*Separate workshop track — see `docs/formal-verification-video-series.md`.*

<!--
Speaker note: One slide only for a general audience. If the room is advanced,
offer to stay after for the FV track. Don't derail the 40-minute talk into SMT.
-->

---

## Source-optional: analyze compiled WASM

You don't always have the source — an audit hands you a `.wasm`.

```bash
sanctifier wasm ./token.wasm
```

- Walks the module: imports, exports, memory, custom `contractspec` sections
- Runs WASM-level checks; reports what it can and **states its limits** vs
  source mode

<br>

Ship an artifact, get a baseline read — no repo required.

<!--
Speaker note: This is the auditor / third-party-integrator slide. Be honest about
limits: bytecode analysis sees less than source. Under-promise here.
-->

---

## Takeaways

1. Soroban has its **own** vulnerability classes — use Soroban-aware tooling
2. Sanctifier turns "hope it's secure" into a **repeatable check**
3. Start today: `analyze` → fix top findings → `baseline` + `diff` in CI
4. Level up: formal verification for the invariants that matter

<br>

**Try it now:** clone the repo, run the workshop lab in `docs/talks/README.md`.

<!--
Speaker note: End with the single next action — run analyze on YOUR contract this
week. One command. Then open the floor / move to the hands-on lab.
-->

---

## Resources & thanks

- **Repo:** Sanctifier (this project)
- **Detector catalog:** `docs/detectors/README.md`
- **Finding codes:** `docs/error-codes.md`
- **Migration / CI guide:** `docs/migration.md`, `docs/ci-cd-setup.md`
- **Demo script for this talk:** `docs/talks/securing-soroban-demo-script.md`
- **Curated ecosystem list:** `docs/awesome-soroban-security.md`

<br>

Questions? → live demo, or find me after.

<!--
Speaker note: Leave this slide up during Q&A so links stay on screen. If running the
workshop, transition here into the hands-on lab and keep the demo script open.
-->
