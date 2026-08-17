#!/bin/bash
# Unit tests for verify_bytecode_hash() in deploy-soroban-testnet.sh, run
# without any network access or a real deployment: a fake `soroban` shell
# function stands in for the CLI and controls whether the "fetched"
# on-chain WASM matches, mismatches, or fails to fetch at all. This is what
# proves the acceptance criterion "Mismatch aborts deploy" actually holds,
# not just that the function exists.
#
# Usage: bash scripts/tests/test-verify-bytecode-hash.sh

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEPLOY_SCRIPT="${SCRIPT_DIR}/../deploy-soroban-testnet.sh"

PASS=0
FAIL=0

setup() {
    WORKDIR=$(mktemp -d)
    export DRY_RUN=false
    export TEMP_DIR="$WORKDIR"
    export DEPLOYMENT_LOG="${WORKDIR}/deployment.log"
    export NETWORK="testnet"
    : > "$DEPLOYMENT_LOG"
}

teardown() {
    rm -rf "$WORKDIR"
    unset -f soroban 2>/dev/null || true
}

assert_success() {
    local desc=$1
    local rc=$2
    if [[ "$rc" -eq 0 ]]; then
        echo "  ok - $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL - $desc (expected success, got exit $rc)"
        FAIL=$((FAIL + 1))
    fi
}

assert_failure() {
    local desc=$1
    local rc=$2
    if [[ "$rc" -ne 0 ]]; then
        echo "  ok - $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL - $desc (expected failure, got exit 0)"
        FAIL=$((FAIL + 1))
    fi
}

# Load function definitions only — the script's own main() is guarded to
# not run when sourced. deploy-soroban-testnet.sh sets `set -euo pipefail`
# at its top, which — because `source` runs in this same shell — would
# otherwise abort this test script on the very first assertion that is
# *supposed* to fail. Restore this script's own error-handling mode
# immediately after sourcing.
# shellcheck source=/dev/null
source "$DEPLOY_SCRIPT"
set +e
set -uo pipefail

echo "verify_bytecode_hash: matching hash passes"
setup
SOURCE_WASM="${WORKDIR}/source.wasm"
printf 'identical bytes' > "$SOURCE_WASM"
soroban() {
    # $1=contract $2=fetch $3=--id $4=<id> $5=--network $6=<net> $7=--out-file $8=<path>
    local out_file=""
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --out-file) out_file="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    cp "$SOURCE_WASM" "$out_file"
}
verify_bytecode_hash "CDEMOCONTRACT1" "$SOURCE_WASM"
assert_success "identical source and on-chain bytes" $?
teardown

echo "verify_bytecode_hash: mismatched hash fails (aborts)"
setup
SOURCE_WASM="${WORKDIR}/source.wasm"
printf 'audited source bytes' > "$SOURCE_WASM"
soroban() {
    local out_file=""
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --out-file) out_file="$2"; shift 2 ;;
            *) shift ;;
        esac
    done
    printf 'a different build entirely' > "$out_file"
}
verify_bytecode_hash "CDEMOCONTRACT2" "$SOURCE_WASM"
assert_failure "mismatched source vs on-chain bytes" $?
teardown

echo "verify_bytecode_hash: fetch failure fails (aborts)"
setup
SOURCE_WASM="${WORKDIR}/source.wasm"
printf 'audited source bytes' > "$SOURCE_WASM"
soroban() { return 1; }
verify_bytecode_hash "CDEMOCONTRACT3" "$SOURCE_WASM"
assert_failure "on-chain fetch failure" $?
teardown

echo "verify_bytecode_hash: dry run always skips (no fetch attempted)"
setup
export DRY_RUN=true
SOURCE_WASM="${WORKDIR}/source.wasm"
printf 'irrelevant' > "$SOURCE_WASM"
soroban() { echo "FAIL: soroban should not be called in dry-run mode" >&2; return 1; }
verify_bytecode_hash "CDEMOCONTRACT4" "$SOURCE_WASM"
assert_success "dry-run skips the network fetch entirely" $?
teardown

echo ""
echo "${PASS} passed, ${FAIL} failed"
[[ "$FAIL" -eq 0 ]]
