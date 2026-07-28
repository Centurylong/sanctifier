# Good First Issue Starter Pack (tracking)

> Pin this issue. Close sub-tasks as individual issues are opened and claimed.

Curated on-ramp for new contributors. Full table: [.github/GOOD_FIRST_ISSUES.md](.github/GOOD_FIRST_ISSUES.md).

## Tasks

- [ ] #1 Add `--quiet` flag to CLI output (`area: cli`, easy)
- [ ] #2 Improve error messages for missing `require_auth` findings (`area: core`, easy)
- [ ] #3 Test `token-with-bugs` auth gap detection (`area: core`, easy)
- [ ] #4 Add `S008` finding: `unwrap_or_default` on auth calls (`area: core`, medium)
- [ ] #5 Add JSON output to `sanctifier analyze` (`area: cli`, easy)
- [ ] #6 Document CLI flags in `docs/cli-reference.md` (`area: docs`, easy)
- [ ] #7 Add `--version` flag (`area: cli`, easy)
- [ ] #8 Proptest coverage for AMM `x * y = k` (`area: contracts`, medium)
- [ ] #9 Fix Clippy warnings in `reentrancy-guard` (`area: contracts`, easy)
- [ ] #10 CI badge in README (`area: docs`, easy)
- [ ] #11 Integration test for runtime-guard health check (`area: contracts`, medium)
- [ ] #12 Add `S009` overflow finding (`area: core`, medium)

## How to claim

1. Comment on the task issue: "I'd like to work on this."
2. Read [CONTRIBUTING.md](../CONTRIBUTING.md) and run `bash scripts/setup.sh` (or use the devcontainer).
3. Open a PR referencing the issue.

Labels: `good first issue`, `help wanted`
