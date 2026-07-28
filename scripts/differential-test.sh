#!/bin/bash

#======================================================================
# Differential-testing harness — Sanctifier vs Slither / Aderyn (issue #503)
#======================================================================
# Runs the two halves of the differential corpus
# (tooling/sanctifier-core/tests/fixtures/corpus):
#
#   1. Sanctifier (always) over the Soroban gallery fixtures, via the Rust
#      harness `differential_test`, which prints the cross-analyzer matrix.
#   2. Slither and/or Aderyn (when installed) over the Solidity mirrors, so the
#      EVM side of "where checks overlap" can be reproduced live.
#
# It degrades gracefully: missing tools are reported and skipped, never fatal.
# Full methodology and results live in docs/differential-testing.md.
#======================================================================

set -uo pipefail

# --- Colors ---
if [ -t 1 ]; then
    BOLD=$(tput bold); GREEN=$(tput setaf 2); YELLOW=$(tput setaf 3); RESET=$(tput sgr0)
else
    BOLD=""; GREEN=""; YELLOW=""; RESET=""
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOLIDITY_DIR="$REPO_ROOT/tooling/sanctifier-core/tests/fixtures/corpus/solidity"

echo "${BOLD}== 1/2 Sanctifier (Soroban corpus) ==${RESET}"
# Use the SMT/Z3 path when the headers are available; otherwise skip that feature
# so the rule-based harness still runs (it does not need Z3).
FEATURES="--all-features"
if ! { [ -f /usr/include/z3.h ] || [ -f /usr/local/include/z3.h ] || [ -f /opt/homebrew/include/z3.h ]; }; then
    echo "${YELLOW}z3.h not found — running detectors without the smt feature.${RESET}"
    FEATURES="--no-default-features"
fi
cargo test -p sanctifier-core $FEATURES --test differential_test -- --nocapture

echo
echo "${BOLD}== 2/2 EVM analyzers (Solidity mirrors) ==${RESET}"

run_evm_tool() {
    local tool="$1"; shift
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "${YELLOW}- $tool not installed — skipping (catalog rows in the matrix above are documentation-only).${RESET}"
        return
    fi
    echo "${GREEN}- $tool found; analyzing Solidity mirrors:${RESET}"
    for sol in "$SOLIDITY_DIR"/*_vulnerable.sol; do
        [ -e "$sol" ] || continue
        echo "  >> $tool $(basename "$sol")"
        case "$tool" in
            slither) "$tool" "$sol" 2>&1 | sed 's/^/     /' || true ;;
            aderyn)  "$tool" "$sol" 2>&1 | sed 's/^/     /' || true ;;
        esac
    done
}

run_evm_tool slither
run_evm_tool aderyn

echo
echo "Done. See ${BOLD}docs/differential-testing.md${RESET} for the analysis and follow-up issues."
