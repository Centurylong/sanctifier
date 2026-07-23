# Demo Script — "Securing Soroban Smart Contracts with Sanctifier"

Live-demo runbook for the talk in
[`securing-soroban-slides.md`](securing-soroban-slides.md). Every command here is
copy-pasteable and runs against a contract that ships in this repo
(`contracts/token-with-bugs`), so the demo works offline with no setup beyond a
built `sanctifier` binary.

> **Presenter tip:** rehearse once end-to-end and paste the *real* finding counts
> from your machine into the `[N findings]` placeholders below — versions evolve,
> and nothing kills credibility like a number that doesn't match the screen.

---

## 0. Before you walk on stage (setup, ~2 min, do it beforehand)

```bash
# Build the CLI once (release is snappier for a live demo).
cargo build -p sanctifier-cli --release

# Put it on PATH for the session so the slides' `sanctifier ...` lines just work.
export PATH="$PWD/target/release:$PATH"

# Sanity check.
sanctifier --version
sanctifier analyze --help | head -20
```

Have two panes open:

- **Left:** an editor on `contracts/token-with-bugs/src/lib.rs`
- **Right:** a terminal at the repo root

Pre-scroll the editor to the `transfer` function. Clear the terminal.

---

## Beat 1 — Meet the patient (slide: *DEMO 1*)

Show the source **before** running anything. Read the three planted bugs aloud.

```bash
sed -n '1,50p' contracts/token-with-bugs/src/lib.rs
```

Point at, in order:

1. **`transfer`** — there is **no `from.require_auth()`**. Any caller can move
   anyone's balance. *(This will be `S001`.)*
2. **`mint`** — `let new_balance = current_balance + amount;` — a plain `+` on
   `i128`. In release, this wraps. *(This will be `S003`.)*
3. **`balance`** — `.unwrap_or(0)` turns "storage entry missing / archived" into a
   real-looking zero balance. *(This will be `SANCT_UNWRAP` / `S002`.)*

> **Say this:** "These aren't exotic. This is what a first draft of a token looks
> like. Let's see what a Soroban-aware analyzer says."

---

## Beat 2 — Run the analyzer (slide: *DEMO 2*)

```bash
sanctifier analyze ./contracts/token-with-bugs
```

Walk the output **top-down**:

- **Summary block** — total findings and a breakdown by severity. Triage rule:
  critical/high first, hygiene later.
- **Findings list** — each line carries a **finding code** (`S001`, `S003`, …), a
  **file:line** location, and a one-line message.

> **Say this:** "Every one of these codes is a page in the catalog — what it
> catches, a vulnerable example, and the fix. The tool isn't just yelling; it's
> teaching."

Cross-reference one finding live:

```bash
# The catalog page for the auth finding we're about to fix.
sed -n '1,40p' docs/detectors/auth_gap.md
```

---

## Beat 3 — JSON output for tooling (slide: *DEMO 2*)

Show that the same run is machine-readable — this is what CI consumes.

```bash
sanctifier analyze ./contracts/token-with-bugs --format json > /tmp/report.json
jq '.summary' /tmp/report.json
jq '.findings | length' /tmp/report.json          # N findings
jq -r '.findings[] | "\(.code)\t\(.message)"' /tmp/report.json | sort | uniq -c
```

> **Say this:** "Text for humans, JSON for pipelines, SARIF for GitHub code
> scanning. Same engine, three audiences."

---

## Beat 4 — Fix one class, watch the count drop (slide: *The fix*)

Edit `contracts/token-with-bugs/src/lib.rs`. Replace the vulnerable `transfer`
(and while we're here, `mint`) with the fixed versions:

```rust
// S001: authorize the party being debited.
// S013: reject zero/negative amounts and self-transfers.
// S003: use checked arithmetic so a balance can never silently wrap.
pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
    from.require_auth();
    assert!(amount > 0, "amount must be positive");
    assert!(from != to, "cannot transfer to self");

    let from_balance = Self::balance(e.clone(), from.clone());
    let new_from = from_balance
        .checked_sub(amount)
        .expect("insufficient balance");
    e.storage().persistent().set(&from, &new_from);

    let to_balance = Self::balance(e.clone(), to.clone());
    let new_to = to_balance
        .checked_add(amount)
        .expect("balance overflow");
    e.storage().persistent().set(&to, &new_to);
}

// S003: checked arithmetic on mint too.
pub fn mint(e: Env, to: Address, amount: i128) {
    to.require_auth();                          // typically admin-gated; simplified
    let current_balance = Self::balance(e.clone(), to.clone());
    let new_balance = current_balance
        .checked_add(amount)
        .expect("balance overflow");
    e.storage().persistent().set(&to, &new_balance);
}
```

Re-run — **this is the moment**:

```bash
sanctifier analyze ./contracts/token-with-bugs
```

> **Say this:** "Same command. The auth and arithmetic findings are gone. We
> didn't argue with the tool — we followed the finding to its catalog page and did
> what it said."

Do it incrementally if time allows: fix `transfer` only, re-run, then `mint`,
re-run. The shrinking count per fix is more persuasive than one big diff.

---

## Beat 5 — Make it a CI gate (slide: *Make it a gate*)

Show how a team adopts this without a "fix everything today" flag day.

```bash
# Accept today's reality as the baseline (the findings you haven't fixed yet).
sanctifier baseline ./contracts/token-with-bugs

# From now on, fail CI only when a change introduces a NEW finding.
sanctifier diff ./contracts/token-with-bugs --baseline .sanctify/baseline.json
```

> **Say this:** "Baseline says 'no new debt.' Diff enforces it on every PR. That's
> how you turn a one-time cleanup into a ratchet that only tightens."

Optionally show the badge / attestation:

```bash
sanctifier badge ./contracts/token-with-bugs
```

---

## Beat 6 — Source-optional: analyze compiled WASM (slide: *Source-optional*)

> **Requires the source-optional WASM mode** (`sanctifier wasm`). If your build
> predates that feature, skip this beat or show a captured transcript — the core
> arc (Beats 1 → 2 → 4 → 5) stands on its own.

For the auditor / integrator audience: you don't always get the repo.

```bash
# Build the contract to WASM, then analyze the artifact directly.
sanctifier wasm ./contracts/token-with-bugs/target/wasm32-unknown-unknown/release/token_with_bugs.wasm
```

> **Say this:** "No source required. It walks the module — imports, exports,
> memory, the Soroban spec sections — runs the checks it can at the bytecode
> level, and it's honest about what it can't see versus source mode."

If you didn't pre-build the WASM, skip the live run and show a captured transcript
instead — building `wasm32-unknown-unknown` mid-talk is a timing risk.

---

## Recovery / fallback

- **A command hangs or errors on stage:** you have `--format json` output saved at
  `/tmp/report.json` from Beat 3 — pivot to reading that with `jq`.
- **Z3 / SMT complaints on a fresh machine:** the detectors in this demo are all
  static; nothing here needs the SMT backend. If a build griped about `z3.h`, the
  detector demo still runs — the formal-verification track is separate.
- **Running long:** cut Beat 3 (JSON) and Beat 6 (WASM); the core arc is
  Beat 1 → 2 → 4 → 5.

---

## Reset between runs

```bash
git checkout -- contracts/token-with-bugs/src/lib.rs
rm -rf .sanctify /tmp/report.json
```

So the next dry-run (or the next session) starts from the same broken baseline.
