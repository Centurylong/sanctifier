# Snapshot Review Workflow

This guide explains how to review and approve insta snapshot diffs for Sanctifier detectors, ensuring transparency in detector changes.

## Overview

Every detector in Sanctifier has a **golden snapshot** of its findings. When detectors change (new rules, refactoring, bug fixes), their output changes and creates snapshot diffs that must be reviewed before merging. This ensures detector changes are intentional and well-understood.

## Quick Start

### Using the Review Script (Recommended)

The `scripts/review-snapshots.ps1` script provides a streamlined workflow:

```powershell
# Run snapshot tests to detect changes
.\scripts\review-snapshots.ps1 -TestOnly

# List pending snapshot files
.\scripts\review-snapshots.ps1 -ListPending

# Interactively review pending changes
.\scripts\review-snapshots.ps1 -Review

# Review only detector snapshots (skip gallery)
.\scripts\review-snapshots.ps1 -Review -DetectorsOnly
```

### Using cargo-insta Directly

```bash
# Install once
cargo install cargo-insta

# Run snapshot tests
cargo insta test -p sanctifier-core --all-features

# Interactively review changes
cargo insta review

# Accept all changes (use with caution)
cargo insta accept

# Reject all changes
cargo insta reject
```

## Understanding Snapshot Changes

### What Triggers a Snapshot Change?

A snapshot diff appears when:
- A detector's logic changes (new rules, modified patterns)
- A detector's output format changes
- A fixture is updated
- A detector is added or removed from the registry

### Reading Snapshot Diffs

Snapshot files are YAML-formatted. A diff shows:

```yaml
# Old snapshot (red lines removed)
- rule_name: arithmetic_overflow
  severity: Warning
  message: "Unchecked '+' operation could overflow"
  location: "deposit:14"

# New snapshot (green lines added)
- rule_name: arithmetic_overflow
  severity: Warning
  message: "Unchecked '+' operation could overflow"
  location: "deposit:14"
  suggestion: Use .checked_add(rhs) or .saturating_add(rhs) to handle overflow
```

**Key changes to watch for:**
- **New findings**: Detector now catches something it missed before (good if intentional)
- **Removed findings**: Detector no longer catches something (potentially a regression)
- **Modified messages**: Wording or severity changes
- **Location changes**: Different line numbers or function names

## Review Process

### 1. Detect Changes

Run snapshot tests to identify what has changed:

```powershell
.\scripts\review-snapshots.ps1 -TestOnly
```

This generates `.snap.new` files for any detectors with changed output.

### 2. Review Each Change

For each pending snapshot:

1. **Identify the detector**: The filename indicates which detector changed (e.g., `detector_snapshots__arithmetic_overflow.snap.new`)

2. **Examine the diff**: Look at what changed in the findings:
   - Are new findings expected from your code change?
   - Did you intentionally remove findings?
   - Are message/suggestion changes improvements?

3. **Check the detector code**: If unsure, look at the detector implementation:
   ```
   tooling/sanctifier-core/src/rules/<detector_name>.rs
   ```

4. **Verify against the fixture**: Check the test fixture:
   ```
   tooling/sanctifier-core/tests/fixtures/detectors/<detector_name>.rs
   ```

### 3. Make a Decision

For each snapshot change, decide:

- **Accept**: The changes are intentional and correct. The detector is working as expected.
- **Reject**: The changes are unintended. Fix the detector code before accepting.
- **Skip**: Defer decision for now (useful when reviewing multiple changes).

### 4. Commit Changes

When accepting changes:
1. Accept the snapshot (script or `cargo insta accept`)
2. Commit both the detector code changes AND the updated `.snap` file
3. In your PR, describe the snapshot changes in the commit message

## Best Practices

### Before Accepting

- **Understand the change**: Don't accept diffs you don't understand
- **Cross-reference**: Check the detector code and fixture to understand why output changed
- **Test manually**: Run the detector against real contracts if possible
- **Consult**: If unsure, ask for a second opinion in a PR review

### Commit Messages

When committing snapshot updates, be descriptive:

```
fix(arithmetic): improve overflow detection with better suggestions

- Add suggestion to use checked_add for overflow cases
- Update snapshot to reflect new suggestion field
- Fixes #123
```

### PR Reviews

When reviewing PRs with snapshot changes:

1. **Check the detector code**: Understand what changed
2. **Review the snapshot diff**: Verify the output change matches the code change
3. **Ask questions**: If the snapshot change is unclear, ask the author to explain
4. **Require documentation**: For significant detector changes, require updated docs

## CI Integration

CI runs snapshot tests in check mode:

```bash
cargo insta test -p sanctifier-core --all-features --check --unreferenced reject
```

- `--check`: Fails the build on any snapshot diff (never writes files)
- `--unreferenced reject`: Fails if a `.snap` has no matching test

This ensures:
- Unreviewed snapshot changes cannot merge
- Orphaned snapshot files are caught
- All detectors have corresponding snapshots

## Troubleshooting

### "Snapshot changes detected" but no `.snap.new` files

Run with `cargo insta test` (without `--check`) to generate pending files.

### Snapshot diff is too large to review

Break down the change:
1. Review the detector code changes first
2. Run tests for a single detector: `cargo test -p sanctifier-core snapshot_<detector_name>`
3. Accept changes incrementally

### Accidentally accepted wrong snapshot

1. Revert the commit that changed the `.snap` file
2. Fix the detector code
3. Re-run the review process

### Gallery snapshots changed but detector didn't

Gallery snapshots run the full registry over the bug gallery. Changes here may indicate:
- A detector was added/removed from the default registry
- A detector's behavior changed in a way that affects multiple bug classes
- The gallery fixtures were updated

Review these carefully as they affect multiple detectors.

## Adding a New Detector

When adding a new detector:

1. Create the detector implementation in `src/rules/`
2. Create a fixture in `tests/fixtures/detectors/<name>.rs`
3. Add a test in `detector_snapshots.rs`
4. Run `cargo insta test -p sanctifier-core --all-features`
5. Review the generated snapshot to ensure it's correct
6. Accept with `cargo insta accept` or the review script
7. Commit the detector, fixture, test, and snapshot together

## Resources

- [Insta documentation](https://insta.rs/)
- [Detector tests README](../tooling/sanctifier-core/tests/README.md)
- [Architecture documentation](ARCHITECTURE.md)
- [Contributing guide](CONTRIBUTING.md)
