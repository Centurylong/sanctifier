# Contributing to Sanctifier

Thanks for your interest. This guide covers everything you need to go from zero to a merged PR.

## Table of Contents

- [Quick start](#quick-start)
- [Contributor on-ramp](#contributor-on-ramp)
- [Dev container (optional)](#dev-container-optional)
- [Project layout](#project-layout)
- [Making changes](#making-changes)
- [Tests and linting](#tests-and-linting)
- [Submitting a PR](#submitting-a-pr)
- [Labels and triage](#labels-and-triage)
- [Good first issues](#good-first-issues)
- [Discussions](#discussions)


---

## Contributor on-ramp

Start here if you want a coherent onboarding experience that links together templates, devcontainer bootstrap, starter pack, roadmap, and docs.

- [Contributor On‑Ramp (One Coherent Start)](docs/contributor-onramp.md)

## Quick start

**Prerequisites:** Rust 1.78+, Git.


```bash
git clone https://github.com/Centurylong/sanctifier.git
cd sanctifier

# Optional: interactive bootstrap (Rust, wasm target, Soroban CLI, deps)
bash scripts/setup.sh

# Build the CLI
cargo build -p sanctifier-cli

# Build sanctifier-core with all features
cargo build -p sanctifier-core --all-features

# Run all tests
cargo test -p sanctifier-core --all-features
cd tooling/sanctifier-cli && cargo test
```

On Ubuntu/Debian you'll also need:

```bash
sudo apt-get install -y libz3-dev libdbus-1-dev
```

Verify the CLI works:

```bash
cargo run -p sanctifier-cli -- analyze ./contracts/vulnerable-contract
```

---

## Dev container (optional)

A `.devcontainer` config is included for VS Code / GitHub Codespaces. It pre-installs Rust, the wasm32 target, Soroban CLI, and all system dependencies.

```
# In VS Code: Ctrl+Shift+P → "Reopen in Container"
# In Codespaces: create a new codespace from the repo page
```

Everything in the Quick start section works the same inside the container.

---

## Project layout

```
tooling/
  sanctifier-core/   # static analysis logic (library)
  sanctifier-cli/    # CLI binary
contracts/           # example / demo Soroban contracts
scripts/             # deployment and CI helpers
frontend/            # Next.js web UI
docs/                # user-facing documentation
```

---

## Making changes

1. Fork the repo and create a branch: `git checkout -b area/short-description`
2. Keep commits focused. One logical change per commit is easiest to review.
3. Run `cargo fmt --all` before committing.

---

## Tests and linting

```bash
# Format check
cargo fmt --all --check

# Clippy (zero warnings policy)
cargo clippy -p sanctifier-core --all-targets --all-features -- -D warnings

# Tests
cargo test -p sanctifier-core --all-features
```

CI runs all of the above on every PR. A red CI is a blocker.

---

## Submitting a PR

- Open the PR against `main`.
- Fill in the PR template — especially the "Closes #" line.
- Keep the diff small where possible. Large PRs take longer to review.
- A maintainer will review within a few days. You may be asked for changes.

---

## Labels and triage

| Label | Meaning |
|---|---|
| `type: bug` | Confirmed bug |
| `type: enhancement` | New feature or improvement |
| `good first issue` | Well-scoped, mentor available |
| `help wanted` | Extra hands welcome |
| `area: core` | sanctifier-core |
| `area: cli` | sanctifier-cli |
| `area: contracts` | Soroban contracts |
| `priority: high` | Should be resolved soon |

---

## Good first issues

Browse [issues labeled `good first issue`](https://github.com/Centurylong/sanctifier/issues?q=is%3Aopen+label%3A%22good+first+issue%22) — each one has a description, learning outcomes, and a mentor listed.

See [.github/GOOD_FIRST_ISSUES.md](.github/GOOD_FIRST_ISSUES.md) and [.github/STARTER_PACK_TRACKING.md](.github/STARTER_PACK_TRACKING.md) for the curated starter pack (pin the tracking issue on GitHub).

---

## Discussions

Use [GitHub Discussions](https://github.com/Centurylong/sanctifier/discussions) for:

- **Q&A** — questions about the codebase or Soroban security
- **Ideas** — proposals before opening a feature request issue
- **Show and tell** — projects built with Sanctifier
- **General** — anything else

Keep issues focused on actionable tasks. Exploratory conversations belong in Discussions.

## Roadmap

Public board: [.github/ROADMAP.md](.github/ROADMAP.md). New issues auto-sync when `ROADMAP_PROJECT_URL` is configured.

## Further reading

- [Awesome Soroban Security](docs/awesome-soroban-security.md)
