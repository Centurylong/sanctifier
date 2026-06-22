# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — 2026-06-18

### Added

- `GuardStatus` enum (`Unlocked = 0`, `Locked = 1`) with `from_u32` constructor.
- `enter_pure(current_status) -> Result<GuardStatus, &'static str>` — pure state-transition
  function; returns `Err("reentrancy detected")` if the guard is already locked.
- `exit_pure() -> GuardStatus` — always returns `Unlocked`.
- `ReentrancyGuard<'a>` — Soroban contract integration using instance storage key `RE_GRD`.
  Exposes `enter()` (panics on reentrancy) and `exit()` (releases the lock).
- `testutils` Cargo feature for test-harness integration with `soroban-sdk/testutils`.
- Four Kani proof harnesses covering all state-machine transitions:
  `verify_enter_fails_when_locked`, `verify_enter_succeeds_when_unlocked`,
  `verify_exit_always_unlocks`, `verify_guard_state_machine`.
- Full crate metadata for publication: `license`, `repository`, `keywords`, `categories`,
  `readme`, `documentation`.
- `contracts/protected-vault` integration example with reentrancy attack scenario test.
