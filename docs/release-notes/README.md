# Release Notes

Monthly summary of what shipped, complementing the
[Public Roadmap](../../.github/ROADMAP.md) (what's planned) with a record of
what's actually landed. Written for users deciding whether to update, and for
contributors who want a changelog-shaped view of the project's pace.

## Cadence

- Published on or around the **1st of each month**, covering the previous
  calendar month.
- A maintainer compiles it from merged PRs (`gh pr list --state merged
  --search "merged:>=YYYY-MM-01"`) and notable closed issues.
- Skipped only if genuinely nothing merged that month — an empty entry isn't
  written just to keep the streak.

## Format

Each entry is a dated file, `YYYY-MM.md`, using the
[template](TEMPLATE.md):

- **Detectors** — new/changed static-analysis rules (link the
  `docs/detectors/<name>.md` page)
- **CLI / tooling** — `sanctifier-cli` changes
- **Docs** — new guides, catalogs, or major doc restructuring
- **Fixes** — notable bug fixes
- **Contributors** — first-time contributors that month (credit is part of
  the point)

## Index

_No entries yet — this directory was just opened. The first monthly entry
lands with the next cadence cycle; see the template below to draft one._

## See also

- [Public Roadmap](../../.github/ROADMAP.md) — what's planned
- [CONTRIBUTING.md](../../CONTRIBUTING.md) — how to get a PR into the next entry
