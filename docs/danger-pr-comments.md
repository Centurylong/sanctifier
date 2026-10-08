# Sanctifier findings in DangerJS pull-request comments

This integration uses the actual output of `sanctifier analyze --format json` (`metadata.format: sanctifier-ci-v1`) to publish concise findings through DangerJS. No GitHub REST posting script, separate auth account or new scanner is required. Danger manages the PR comment; high/critical findings are `fail`, ordinary findings are `warn`, and low/informational findings are `message`.

## Install and run

1. Ensure `sanctifier` is on PATH and can analyze the Soroban project you want to check. For this repository, build the CLI with `cargo build --release -p sanctifier-cli` and add `target/release` to PATH.
2. Add `danger` as a development dependency in the project running CI (for example `npm install --save-dev danger`). DangerJS must have a PR-aware CI environment and a token with permission to publish PR comments.
3. Copy `integrations/danger/sanctifier-danger.cjs` into the target repository. Copy `integrations/danger/example/Dangerfile.cjs` alongside it, updating the relative `require("../sanctifier-danger.cjs")` path if moved.
4. Run `npx danger ci --dangerfile integrations/danger/example/Dangerfile.cjs` after the CLI build. Set `SANCTIFIER_TARGET` to the project directory (defaults to `.`); optionally set `SANCTIFIER_BIN` to an absolute CLI path.

Example GitHub Actions job for a repository containing this integration:

```yaml
name: Sanctifier PR findings
on: pull_request
permissions:
  contents: read
  pull-requests: write
jobs:
  danger:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '22'
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo build --release -p sanctifier-cli
      - run: npm install --no-save danger
      - run: npx danger ci --dangerfile integrations/danger/example/Dangerfile.cjs
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          SANCTIFIER_BIN: ./target/release/sanctifier
          SANCTIFIER_TARGET: ./contracts/vulnerable-contract
```

Adjust the target path to a Soroban project in your repository. On forked PRs, default GitHub tokens may not have PR-write permissions; use the repository's approved PR-comment publishing workflow, not secrets passed to untrusted PR code. For an external project, install the CLI and adjust build/path steps accordingly.

## Semantics and safety

- Parses only `sanctifier-ci-v1`; malformed or unsuccessful scanner JSON is a Danger failure, **not** a clean scan.
- Exit status 1 with a valid JSON report is accepted: Sanctifier uses it for high/critical findings. Other unsuccessful execution statuses are reported as scan failure.
- Reads all 11 typed `findings` groups plus top-level `vulnerability_db_matches`, which would otherwise be omitted. Honors the scanner's already-applied baseline; the Danger summary includes the suppression count.
- Emits at most 12 individual Danger annotations by default, always with one bounded Markdown report (first 30 rows). This avoids hundreds of comments on large scans. Override with `maxAnnotations` 1–50 in Dangerfile.
- Anchors an inline annotation only when an actual Rust file + positive line are known and the file appears in Danger's changed-file list. Other findings appear on the PR without an unsafe/invented line.
- Does not itself implement a base-vs-head diff: 'new' means new relative to the active Sanctifier baseline. Without a baseline, the report can contain pre-existing findings.
- Uses `spawnSync` without a shell, so target/project paths are arguments, not interpreted commands. CLI diagnostics aren't echoed into public PR comments.

## Example and focused smoke

`integrations/danger/example/sample-report.json` follows the scanner's nested fields, including a high-severity panic, a warning custom rule and a low-severity vulnerability DB match. Run:

```bash
node integrations/danger/example/smoke.cjs
```

This runs the plugin with mock Danger actions and checks status counts, an inline changed-file location, the baseline summary, and a malformed-report rejection. It does not require a Rust build, GitHub token or running Danger's full CI. To smoke the full posting integration, invoke DangerJS in a real example PR; no live PR comment or end-to-end CI success is claimed by this local check.
