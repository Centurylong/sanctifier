# Reentrancy Guardian

## Overview

The **Reentrancy Guardian** is a reusable Soroban smart contract template that provides
two complementary layers of reentrancy protection for complex multi-step workflows.

## Why this exists

Soroban natively prevents **classical cross-contract reentrancy**: a contract cannot be
re-entered while its current execution frame is still on the host stack. However,
**state-based reentrancy** within complex workflows remains possible:

> An attacker-controlled contract could observe intermediate, partially-updated state
> during a complex workflow and call a _different_ entry-point on the same contract
> that depends on that unstable state — before the original invocation has finalized.

## Contract location

```
contracts/reentrancy-guardian/
├── Cargo.toml
└── src/
    └── lib.rs          ← ReentrancyGuardian contract + tests
```

## How it works

The Guardian uses **runtime nonces** stored in Soroban instance storage to enforce
strictly sequential, non-overlapping workflow execution.

### Layer 1 — Lock Guard

A boolean flag (`DataKey::Lock`) that is set to `true` on `enter` and `false` on
`exit`. Any attempt to `enter` while the flag is already `true` is immediately rejected
with `Error::Locked`. This mirrors the classic mutex/reentrancy-lock pattern but on
persistent contract storage.

### Layer 2 — Nonce Guard (state-based reentrancy prevention)

A monotonically-increasing `u64` counter (`DataKey::Nonce`) stored in instance storage.
The caller **must supply the exact current nonce** when calling `enter`. The contract
immediately increments it on success.

This prevents:

- **Replay attacks** — a captured `enter` call cannot be replayed (nonce already advanced).
- **State-based reentrancy** — even if an attacker can observe the state between `enter`
  and `exit`, they cannot enter the same logical slot because the nonce has already
  moved forward.

## Public interface

```rust
// Initialize the Guardian (called once at contract setup)
guardian.init();

// Enter a guarded section — panics if locked or nonce is wrong
guardian.enter(nonce: u64);

// Exit a guarded section — releases the lock
guardian.exit();

// Read the current nonce (call this before `enter` to obtain the correct value)
guardian.get_nonce() -> u64;
```

## Error codes

| Error             | Code | Meaning                                                        |
| ----------------- | ---- | -------------------------------------------------------------- |
| `Error::Locked`   | 1    | `enter` called while lock is already active (re-entry attempt) |
| `Error::Mismatch` | 2    | Provided nonce does not match the contract's current nonce     |

## Usage pattern (parent contract)

```rust
// 1. Read the current nonce
let nonce = guardian_client.get_nonce();

// 2. Enter the guarded section (atomically validates + increments nonce)
guardian_client.enter(&nonce);

// 3. Perform sensitive state changes / external calls
token_client.transfer(&from, &to, &amount);
env.storage().instance().set(&DataKey::Balance, &new_balance);

// 4. Release the lock
guardian_client.exit();
```

## Running tests

```bash
cargo test -p reentrancy-guardian
```

Expected output:

```
running 4 tests
test test::test_exit_releases_lock ... ok
test test::test_lock_blocks_reentry ... ok
test test::test_nonce_mismatch_fails ... ok
test test::test_standard_flow ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Static analysis detection

The `sanctifier-core` analyzer includes enhanced static analysis that detects multiple
risky reentrancy patterns in contract functions:

### Detected Patterns

1. **External Call in Loop** (High Severity)
   - External calls inside `for`, `while`, or `loop` constructs
   - Can lead to reentrancy attacks and excessive gas consumption

2. **Multiple External Calls** (High Severity)
   - Functions making 2+ external calls without reentrancy protection
   - Increases attack surface for complex reentrancy scenarios

3. **State After Call** (High Severity)
   - State mutations occurring after external calls
   - Violates Checks-Effects-Interactions (CEI) pattern

4. **Critical CEI Violation** (High Severity)
   - State mutations both before AND after external calls
   - Most dangerous pattern - state can be manipulated mid-execution

5. **State Before Call** (Medium Severity)
   - Classic reentrancy pattern: state mutation followed by external call
   - Should use reentrancy guard or follow CEI pattern

### Usage

Run via the CLI:

```bash
sanctifier analyze ./my-contract
```

Example findings:

```
🔄 Reentrancy Risk Detected!
   -> Function `batch_transfer`: External call in loop (HIGH)
      💡 External calls in loops can lead to reentrancy attacks and gas issues.
         Consider batching operations or using a reentrancy guard.

🔄 Reentrancy Risk Detected!
   -> Function `complex_workflow`: Multiple external calls without guard (HIGH)
      💡 Function makes 3 external calls without reentrancy protection.
         Use ReentrancyGuardian.enter(nonce) / .exit() to protect this function.

🔄 Reentrancy Risk Detected!
   -> Function `risky_update`: State mutation after external call (HIGH)
      💡 State mutation after external call violates Checks-Effects-Interactions pattern.
         Move state changes before the external call or use ReentrancyGuardian.
```

### Pattern Detection Details

The analyzer tracks:

- Statement execution order to detect CEI violations
- Loop contexts to identify calls in iterations
- Multiple external call sequences
- Various guard naming patterns (`guardian`, `guard`, `reentrancy_lock`)
- Different external call methods (`client.*`, `invoke_contract`, etc.)

## Security notes

- The Guardian stores its state in **instance storage**, which is scoped to the contract's
  own ledger entry and survives across transactions.
- Soroban does not persist in-memory state between invocations: every guard cycle must
  call `enter` → do work → `exit` within a single transaction.
- The Lock flag is automatically released on `exit`. If your workflow aborts mid-execution
  (panic / error), the lock will still be set on the next transaction. Ensure your parent
  contract always calls `exit` in a cleanup path, or implement an admin `force_reset`.
