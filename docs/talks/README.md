# Talks & Workshops

Reusable, ready-to-present material for demonstrating **Sanctifier** at
conferences, meetups, and hands-on workshops. Everything here runs against
contracts that already ship in this repo — no external setup, works offline.

> Filed for issue #777 (*Conference talk / workshop material on securing
> Soroban*). Acceptance criteria: **slides + demo script published** — both live
> in this directory.

## Contents

| File | What it is |
|---|---|
| [`securing-soroban-slides.md`](securing-soroban-slides.md) | The slide deck (Markdown, Marp-compatible). Speaker notes in HTML comments. |
| [`securing-soroban-demo-script.md`](securing-soroban-demo-script.md) | The live-demo runbook — copy-pasteable commands, beat by beat, with a fallback plan. |

## Formats

- **40-minute conference talk** — present the deck, run the demo beats inline
  (Beats 1 → 2 → 4 → 5). Total ≈ 40 min with Q&A.
- **90-minute hands-on workshop** — same deck, but attendees run each demo beat
  themselves on their own machines. Budget: 15 min intro, 60 min guided labs
  (the six beats), 15 min CI wire-up + questions.

## Presenting the slides

The deck is plain Markdown with slides separated by `---`, so it renders with any
Markdown-slides tool:

```bash
# Marp (recommended — respects the front-matter and speaker notes)
marp docs/talks/securing-soroban-slides.md --html -o securing-soroban-slides.html

# or reveal-md
reveal-md docs/talks/securing-soroban-slides.md

# or just open it in the "Marp for VS Code" extension preview.
```

Speaker notes for each slide live in `<!-- ... -->` comments and surface in Marp's
presenter view.

## Running the demo

Follow [`securing-soroban-demo-script.md`](securing-soroban-demo-script.md). In
short:

```bash
cargo build -p sanctifier-cli --release
export PATH="$PWD/target/release:$PATH"
sanctifier analyze ./contracts/token-with-bugs        # find the planted bugs
# ...fix transfer/mint per the script...
sanctifier analyze ./contracts/token-with-bugs        # watch the count drop
```

The demo target, `contracts/token-with-bugs`, has three intentional
vulnerabilities (missing auth, unchecked arithmetic, storage-default masking) that
map cleanly to detector catalog pages.

## Workshop lab checklist

Give attendees this as a handout:

1. **Setup** — build the CLI, put it on `PATH`, run `sanctifier --version`.
2. **Lab 1 — Read a report.** Run `analyze` on `contracts/token-with-bugs`;
   identify each finding code and open its page under
   [`docs/detectors/`](../detectors/README.md).
3. **Lab 2 — Fix the auth gap (`S001`).** Add `from.require_auth()`; re-run;
   confirm the finding is gone.
4. **Lab 3 — Fix the arithmetic (`S003`).** Switch to `checked_add`/`checked_sub`;
   re-run.
5. **Lab 4 — Gate CI.** `baseline`, then `diff` — introduce a new bug and watch
   `diff` fail.
6. **Stretch — Source-optional.** Build to WASM and run `sanctifier wasm` on the
   artifact.

## Adapting the material

- **Shorter (lightning, 10 min):** slides "Why Soroban is different" →
  "What Sanctifier is" → DEMO 2 → "Make it a gate" → takeaways. One fix, live.
- **Audit-focused audience:** lead with the source-optional / WASM slide and
  Beat 6; deemphasize CI.
- **Your own contract:** swap `contracts/token-with-bugs` for the attendee's
  project throughout — every command takes an arbitrary path.

## Keeping it current

Finding codes and detector names can change as the tool evolves. Before a talk:

- Re-run the demo and paste the real counts into the script's `[N findings]`
  placeholders.
- Cross-check codes against [`docs/error-codes.md`](../error-codes.md) and the
  [detector catalog](../detectors/README.md).
- Confirm CLI flags against [`docs/cli.md`](../cli.md) (auto-generated, always
  in sync with the parser).

## See also

- [Detector catalog](../detectors/README.md) — one page per detector.
- [Finding codes](../error-codes.md) — `S001…S017` reference.
- [Migration guide](../migration.md) & [CI/CD setup](../ci-cd-setup.md) — the
  "make it a gate" story in depth.
- [Formal-verification video series](../formal-verification-video-series.md) — the
  companion FV workshop track.
