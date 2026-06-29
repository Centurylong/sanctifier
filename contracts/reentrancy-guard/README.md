# reentrancy-guard

A formally verified, `#![no_std]`-compatible reentrancy guard for
[Soroban](https://stellar.org/soroban) smart contracts.

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](../../LICENSE)

## Overview

Reentrancy attacks occur when a malicious contract calls back into the victim contract before
the first invocation completes. This crate provides a lightweight guard that stores a lock
flag in Soroban instance storage, preventing any reentrant call from proceeding.

The state-transition core (`enter_pure` / `exit_pure`) is verified exhaustively with
[Kani](https://model-checking.github.io/kani/), giving mathematical certainty that the
state machine is correct for all possible inputs.

## Installation

Add to your contract's `Cargo.toml`:

```toml
[dependencies]
reentrancy-guard = "0.1"
```

## Quick Start

```rust
use reentrancy_guard::ReentrancyGuard;
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct MyVault;

#[contractimpl]
impl MyVault {
    pub fn withdraw(env: Env, amount: i128) {
        let guard = ReentrancyGuard::new(&env);
        guard.enter();          // panics with "reentrancy detected" if re-entered

        // … safely perform the withdrawal …

        guard.exit();
    }
}
```

The guard stores a single `u32` flag under the storage key `RE_GRD` in Soroban instance
storage. Calling `enter()` while the flag is already set causes an immediate panic, aborting
the transaction.

## API

### `GuardStatus`

```rust
pub enum GuardStatus {
    Unlocked = 0,
    Locked   = 1,
}

impl GuardStatus {
    pub fn from_u32(val: u32) -> Self;
}
```

Represents the state of the guard in storage.

---

### `enter_pure(current_status: GuardStatus) -> Result<GuardStatus, &'static str>`

Pure state-transition function with no side effects. Returns `Ok(Locked)` when transitioning
from `Unlocked`, or `Err("reentrancy detected")` if already `Locked`. This is the target of
Kani formal verification.

---

### `exit_pure() -> GuardStatus`

Always returns `GuardStatus::Unlocked`. Call this after the protected section completes.

---

### `ReentrancyGuard<'a>`

```rust
impl<'a> ReentrancyGuard<'a> {
    /// Create a new guard bound to the contract environment.
    pub fn new(env: &'a Env) -> Self;

    /// Enter a protected section. Panics on reentrancy.
    pub fn enter(&self);

    /// Exit a protected section, releasing the lock.
    pub fn exit(&self);
}
```

## Security Properties

| Property | Status | Verified by |
|---|---|---|
| Cannot enter a locked section | Proven | `verify_enter_fails_when_locked` |
| Entering an unlocked section succeeds | Proven | `verify_enter_succeeds_when_unlocked` |
| Exit always unlocks | Proven | `verify_exit_always_unlocks` |
| Full state-machine coverage | Proven | `verify_guard_state_machine` |

## Kani Formal Verification

The pure logic is verified with four Kani proof harnesses:

```
verify_enter_fails_when_locked       — entering a locked guard always errors
verify_enter_succeeds_when_unlocked  — entering an unlocked guard always succeeds
verify_exit_always_unlocks           — exit always sets state to Unlocked
verify_guard_state_machine           — exhaustive two-state transition check
```

To run verification locally (requires [Kani](https://model-checking.github.io/kani/install-guide.html)):

```bash
cd contracts/reentrancy-guard
cargo kani
```

## Integration Example

See [`contracts/protected-vault`](../protected-vault) in this repository for a complete Soroban
vault contract that uses this guard and includes an integration test demonstrating that a
simulated reentrancy attack is caught and rejected.

## License

MIT
