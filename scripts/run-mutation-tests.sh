#!/bin/bash
# Runs `cargo mutants` against sanctifier-core's detector rules (see
# .cargo/mutants.toml for scope) and writes a report.
#
# Usage:
#   scripts/run-mutation-tests.sh                                    # full scoped run
#   scripts/run-mutation-tests.sh --file tooling/sanctifier-core/src/rules/division_by_zero.rs
#   (any extra args are passed straight through to `cargo mutants`)
#
# See docs/mutation-testing.md for what to do with the results. Run from
# the workspace root — `cargo mutants` discovers .cargo/mutants.toml (and
# its workspace-relative examine_globs) relative to the workspace root,
# not the sanctifier-core crate directory.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
OUT_DIR="${PROJECT_ROOT}/mutants.out"

if ! command -v cargo-mutants &> /dev/null; then
    echo "cargo-mutants not found. Installing (cargo install --locked cargo-mutants)..."
    cargo install --locked cargo-mutants
fi

cd "$PROJECT_ROOT"

echo "Running cargo mutants (package: sanctifier-core, scope: see .cargo/mutants.toml)..."
cargo mutants -p sanctifier-core --no-shuffle "$@"

echo ""
echo "Report written to: ${OUT_DIR}/outcomes.json"
echo "Human-readable log: ${OUT_DIR}/caught.txt (caught) and missed.txt (survivors)"
if [[ -f "${OUT_DIR}/missed.txt" ]]; then
    survivor_count=$(wc -l < "${OUT_DIR}/missed.txt" | tr -d ' ')
    echo ""
    echo "${survivor_count} surviving mutant(s) — see docs/mutation-testing.md for triage guidance."
fi
