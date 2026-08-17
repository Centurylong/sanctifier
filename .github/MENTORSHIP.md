# Mentorship Labels + Triage

Sanctifier's contributor wave (GrantFox OSS campaigns) brings in a burst of
new contributors at once. This doc defines the labels and the weekly triage
process that keep that wave moving instead of stalling on "who do I ask?".

It complements [`.github/GOOD_FIRST_ISSUES.md`](GOOD_FIRST_ISSUES.md) (the
curated task list) and [`docs/contributor-onramp.md`](../docs/contributor-onramp.md)
(environment setup) — this doc is specifically about *matching* a new
contributor to a mentor and *unblocking* them if they go quiet.

## Labels

| Label | Meaning | Applied by |
| --- | --- | --- |
| `mentor: available` | A maintainer has volunteered to mentor whoever claims this issue | Maintainer, at triage |
| `mentor: assigned` | A specific mentor is now paired with the assignee (see the issue comment for who) | Maintainer, on claim |
| `status: needs-triage` | New issue not yet reviewed for difficulty/mentor fit | Auto (issue template default) |
| `status: claimed` | Someone has commented "I'd like to work on this" and is awaiting assignment | Maintainer, within 48h |
| `status: stalled` | Claimed >10 days ago with no PR or update comment | Maintainer, at weekly triage |
| `difficulty: easy` / `medium` / `hard` | Existing scope labels — see `GOOD_FIRST_ISSUES.md` | Maintainer, at triage |

These are additive to the existing `area:*`, `priority:*`, and `type:*`
labels already used across the tracker; they don't replace them.

## Triage process (weekly)

1. **Sweep `status: needs-triage`.** For each new issue: confirm it's
   well-scoped (single focused PR — split it if not), assign a
   `difficulty:*` label, and decide if it needs `mentor: available`
   (recommended for anything touching `tooling/sanctifier-core/src/rules/`
   or the AST-visitor patterns — the highest-friction area for newcomers).
2. **Sweep `status: claimed`.** Anything claimed >48h with no assignment yet
   gets assigned now.
3. **Sweep `status: stalled`.** Anything claimed >10 days with no PR or
   comment: leave a "still working on this?" comment, and if there's no
   response within 3 more days, remove the assignment and reopen the claim
   so someone else can pick it up.

## Becoming a mentor

Comment on any unclaimed issue with `mentor: available` (or ask a maintainer
to add the label): "I can mentor this one." When someone claims it, reply
with how to reach you (GitHub comments, or a channel from
[Discussions](https://github.com/Centurylong/sanctifier/discussions)) and
what "done" looks like — link the fixture/snapshot/docs checklist in
[`docs/detector-authoring-guide.md`](../docs/detector-authoring-guide.md) if
the task is a new detector.

## See also

- [Good First Issue Starter Pack](GOOD_FIRST_ISSUES.md)
- [Contributor On-Ramp](../docs/contributor-onramp.md)
- [CONTRIBUTING.md](../CONTRIBUTING.md)
