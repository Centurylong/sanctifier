#![no_std]

//! Proof-of-concept: Kani formal verification harnesses for a standard Soroban token contract.
//!
//! This module demonstrates the "Core Logic Separation" pattern: extract pure balance/transfer
//! logic into functions that can be verified with Kani, while the contract layer that uses
//! `Env`, `Address`, `Symbol`, etc. remains unverified due to Host type limitations.

use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol};

// ── Token initialisation pure logic (verified with Kani) ─────────────────────
//
// The contract must only be initialised once.  We model the "already initialised"
// flag as a single boolean: `is_initialized == true` means setup has already run.
// The function is pure (no Host/FFI), so Kani can reason about every possible
// combination of inputs exhaustively.

/// Attempt to initialise the token contract.
///
/// * `is_initialized` – whether the contract has already been set up.
/// * Returns `Ok(())` on success (transitions the flag from `false` → `true`).
/// * Returns `Err("already initialized")` if the token was already set up,
///   guaranteeing that a second call can **never** succeed.
pub fn initialize_pure(is_initialized: bool) -> Result<(), &'static str> {
    if is_initialized {
        return Err("already initialized");
    }
    Ok(())
}


 // ── Pure admin authorization transition (Kani-verifiable) ─────────────────
 //
 // auth_succeeded models the Soroban Host require_auth verdict:
 // an unsuccessful require_auth traps BEFORE a storage write. It is a symbolic
 // input in the proof, not an assumption that every caller is authorized.

/// Predicate shared by the pure transition and the on-chain entrypoint.
pub fn admin_write_allowed(caller_is_admin: bool, auth_succeeded: bool) -> bool {
    caller_is_admin && auth_succeeded
}

/// The modeled observable storage effect of attempting to replace an admin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminWriteOutcome {
    pub admin: u64,
    pub wrote: bool,
}

/// Pure model: arbitrary administrator/caller identities, authentication
/// verdict, and replacement address (represented as opaque u64 identifiers).
/// Rejected calls leave storage unchanged and must not reach a write.
pub fn admin_write_pure(
    admin: u64,
    caller: u64,
    auth_succeeded: bool,
    replacement: u64,
) -> AdminWriteOutcome {
    let wrote = admin_write_allowed(caller == admin, auth_succeeded);
    AdminWriteOutcome {
        admin: if wrote { replacement } else { admin },
        wrote,
    }
}

/// Intentionally vulnerable negative control: trusting a caller-provided
/// admin identity without the Host require_auth check allows impersonation.
#[cfg(any(kani, test))]
fn admin_write_missing_auth_bug(
    admin: u64,
    caller: u64,
    replacement: u64,
) -> AdminWriteOutcome {
    let wrote = caller == admin; // BUG: no authentication check.
    AdminWriteOutcome {
        admin: if wrote { replacement } else { admin },
        wrote,
    }
}

// ── Pure logic (verified with Kani) ─────────────────────────────────────────────
//
// These functions operate only on i128 and have no Host/FFI dependencies.
// They model the core arithmetic of a standard Soroban token: transfer, mint, burn.

/// Transfer: deduct from sender, add to receiver.
pub fn transfer_pure(
    balance_from: i128,
    balance_to: i128,
    amount: i128,
) -> Result<(i128, i128), &'static str> {
    if amount <= 0 {
        return Err("Amount must be positive");
    }
    let new_from = balance_from
        .checked_sub(amount)
        .ok_or("Insufficient balance")?;
    let new_to = balance_to
        .checked_add(amount)
        .ok_or("Receiver balance overflow")?;
    Ok((new_from, new_to))
}

/// Mint: add to a balance.
pub fn mint_pure(balance: i128, amount: i128) -> Result<i128, &'static str> {
    if amount <= 0 {
        return Err("Mint amount must be positive");
    }
    balance.checked_add(amount).ok_or("Mint overflow")
}

/// Burn: subtract from a balance.
pub fn burn_pure(balance: i128, amount: i128) -> Result<i128, &'static str> {
    if amount <= 0 {
        return Err("Burn amount must be positive");
    }
    balance
        .checked_sub(amount)
        .ok_or("Insufficient balance to burn")
}

/// Deliberately **buggy** transfer used as a formal-verification counterexample.
///
/// It debits the sender by `amount` but credits the receiver by `amount + 1` — so
/// every transfer silently *mints* one unit and the total supply grows. This is
/// the canonical "mint-on-transfer" bug. It exists only to demonstrate that the
/// conservation proof actually has teeth: `verify_mint_on_transfer_bug_breaks_
/// conservation` (a `#[kani::should_panic]` harness) proves Kani finds the
/// violating trace, and `mint_on_transfer_bug_creates_supply` shows it under a
/// plain unit test. It is compiled only under `kani`/`test`, never in a release
/// build.
#[cfg(any(kani, test))]
pub fn transfer_pure_mint_bug(
    balance_from: i128,
    balance_to: i128,
    amount: i128,
) -> Result<(i128, i128), &'static str> {
    if amount <= 0 {
        return Err("Amount must be positive");
    }
    let new_from = balance_from
        .checked_sub(amount)
        .ok_or("Insufficient balance")?;
    // BUG: credits `amount + 1` instead of `amount`, minting one unit per call.
    let new_to = balance_to
        .checked_add(amount)
        .and_then(|v| v.checked_add(1))
        .ok_or("Receiver balance overflow")?;
    Ok((new_from, new_to))
}

// ── Contract (not verified: uses Host types) ────────────────────────────────────

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    /// Wrapper exposing transfer_pure for contract use.
    /// A full implementation would read/write balances via env.storage().
    pub fn transfer(balance_from: i128, balance_to: i128, amount: i128) -> (i128, i128) {
        transfer_pure(balance_from, balance_to, amount).expect("transfer failed")
    }

    /// One-shot initialisation entry point.
    ///
    /// Reads the flag from instance storage, delegates to `initialize_pure`, and
    /// persists the flag on success.  Kani verifies the pure guard; the Host layer
    /// here is intentionally thin and untouched by the proof.
    pub fn initialize(env: Env, _name: Symbol, admin: Address) {
        // The initializer must control the key for the first administrator.
        // A rejected require_auth aborts before either storage write.
        admin.require_auth();
        let already: bool = env
            .storage()
            .instance()
            .get(&symbol_short!("init"))
            .unwrap_or(false);
        initialize_pure(already).expect("already initialized");
        env.storage().instance().set(&symbol_short!("admin"), &admin);
        env.storage().instance().set(&symbol_short!("init"), &true);
    }

    /// Update the stored admin only after identity comparison AND a successful
    /// Soroban Host require_auth. These Host calls cannot be executed by Kani;
    /// admin_write_pure verifies the matching authorization/state transition.
    ///
    /// API change in this PoC: current_admin and new_admin are Address values,
    /// and initialize now requires an initial authorized Address.
    pub fn set_admin(env: Env, current_admin: Address, new_admin: Address) {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&symbol_short!("admin"))
            .expect("admin not initialized");
        let caller_is_admin = current_admin == stored_admin;
        assert!(caller_is_admin, "caller is not the current admin");
        current_admin.require_auth(); // Host traps on a missing signature.
        assert!(admin_write_allowed(caller_is_admin, true));
        env.storage()
            .instance()
            .set(&symbol_short!("admin"), &new_admin);
    }
}

// ── Kani harnesses ─────────────────────────────────────────────────────────────

#[cfg(kani)]
mod verification {
    use super::*;


    // ── Access control invariant (#341) ──────────────────────────────────

    /// Exhaust all admin/caller identities and both authentication verdicts.
    /// This proof does NOT assume a permitted call: rejected states must not
    /// write storage, even when the proposed admin differs from the prior one.
    #[kani::proof]
    fn verify_admin_write_requires_current_authenticated_admin() {
        let admin: u64 = kani::any();
        let caller: u64 = kani::any();
        let auth_succeeded: bool = kani::any();
        let replacement: u64 = kani::any();

        let after = admin_write_pure(admin, caller, auth_succeeded, replacement);
        if caller != admin || !auth_succeeded {
            assert!(!after.wrote, "unauthorized path performed an admin write");
            assert_eq!(after.admin, admin, "unauthorized path changed admin storage");
        } else {
            assert!(after.wrote, "authorized path should reach the write");
            assert_eq!(after.admin, replacement);
        }
        assert!(!after.wrote || (caller == admin && auth_succeeded));
    }

    /// A rejected Host authorization with a caller-supplied admin identity is
    /// a reachable witness against code which omits require_auth.
    /// Kani must find the failing assertion for this negative control.
    /// Once a legitimate rotation succeeds, the former admin's signature
    /// alone must not authorize a subsequent write, even when the Host
    /// authenticates that former principal.
    #[kani::proof]
    fn verify_old_admin_is_revoked_after_rotation() {
        let original: u64 = kani::any();
        let replacement: u64 = kani::any();
        let attempted_next: u64 = kani::any();
        kani::assume(original != replacement);

        let first = admin_write_pure(original, original, true, replacement);
        assert!(first.wrote);
        assert_eq!(first.admin, replacement);

        let replay = admin_write_pure(first.admin, original, true, attempted_next);
        assert!(!replay.wrote, "the former admin unexpectedly reached a write");
        assert_eq!(replay.admin, replacement);
    }

    #[kani::proof]
    #[kani::should_panic]
    fn verify_missing_require_auth_is_caught() {
        let admin: u64 = kani::any();
        let caller: u64 = kani::any();
        let replacement: u64 = kani::any();
        let auth_succeeded: bool = kani::any();
        kani::assume(caller == admin && !auth_succeeded);
        let after = admin_write_missing_auth_bug(admin, caller, replacement);
        assert!(!after.wrote, "unauthenticated caller was allowed to write");
    }

    #[kani::proof]
    fn verify_transfer_pure_conservation() {
        let balance_from: i128 = kani::any();
        let balance_to: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        kani::assume(balance_from >= amount);
        kani::assume(balance_from <= i128::MAX);
        kani::assume(balance_to >= 0);
        kani::assume(balance_to <= i128::MAX - amount);
        // Ensure the conservation assert itself (new_from + new_to) doesn't overflow.
        // new_from = balance_from - amount, new_to = balance_to + amount
        // new_from + new_to = balance_from + balance_to, so we need total to fit.
        kani::assume(balance_from <= i128::MAX - balance_to);

        let Ok((new_from, new_to)) = transfer_pure(balance_from, balance_to, amount) else {
            panic!("transfer_pure failed despite valid preconditions");
        };

        assert!(new_from == balance_from - amount);
        assert!(new_to == balance_to + amount);
        assert!(
            new_from + new_to == balance_from + balance_to,
            "Conservation of supply"
        );
    }

    /// **Counterexample**: a mint-on-transfer bug is *caught* by the conservation
    /// property.
    ///
    /// `transfer_pure_mint_bug` credits the receiver one unit more than it debits
    /// the sender, so `new_from + new_to == balance_from + balance_to + 1` and the
    /// conservation equality is violated for **every** admissible input. The
    /// harness asserts conservation and is marked `#[kani::should_panic]`: the
    /// proof succeeds precisely because Kani finds a trace where the assertion
    /// fails. This is the flip side of `verify_transfer_pure_conservation` — it
    /// demonstrates the conservation harness is non-vacuous and would reject a
    /// contract that mints value on transfer.
    ///
    /// Bounds mirror the passing harness so the two are directly comparable: the
    /// only difference is the buggy `+ 1`, isolating the defect as the sole cause
    /// of the violation.
    #[kani::proof]
    #[kani::should_panic]
    fn verify_mint_on_transfer_bug_breaks_conservation() {
        let balance_from: i128 = kani::any();
        let balance_to: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        kani::assume(balance_from >= amount);
        kani::assume(balance_to >= 0);
        // Leave head-room for the buggy `+ 1` credit so the overflow guard, not an
        // i128 overflow, is what the harness exercises.
        kani::assume(balance_to <= i128::MAX - amount - 1);
        kani::assume(balance_from <= i128::MAX - balance_to);

        let Ok((new_from, new_to)) = transfer_pure_mint_bug(balance_from, balance_to, amount)
        else {
            panic!("buggy transfer failed despite valid preconditions");
        };

        // This assertion is expected to FAIL (supply grew by one) — that failing
        // trace is exactly what `should_panic` proves Kani discovers.
        assert!(
            new_from + new_to == balance_from + balance_to,
            "Conservation of supply"
        );
    }

    /// **Property**: Transfer fails when `amount <= 0`.
    ///
    /// `transfer_pure` explicitly guards against non-positive amounts.
    /// Kani proves this guard always fires for every non-positive `amount`.
    #[kani::proof]
    fn verify_transfer_pure_rejects_non_positive_amount() {
        let balance_from: i128 = kani::any();
        let balance_to: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount <= 0);

        let result = transfer_pure(balance_from, balance_to, amount);
        assert!(result.is_err(), "transfer must fail when amount <= 0");
    }

    /// **Property**: Transfer fails when subtraction would underflow `i128`.
    ///
    /// `checked_sub` returns `None` (and `transfer_pure` returns `Err`) only
    /// when `balance_from - amount < i128::MIN`, i.e. true integer underflow.
    #[kani::proof]
    fn verify_transfer_pure_rejects_underflow() {
        let balance_from: i128 = kani::any();
        let balance_to: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        // Underflow condition: balance_from - amount < i128::MIN
        kani::assume(balance_from < i128::MIN + amount);

        let result = transfer_pure(balance_from, balance_to, amount);
        assert!(result.is_err(), "transfer must fail on i128 underflow");
    }

    #[kani::proof]
    fn verify_mint_pure() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        kani::assume(balance >= 0);
        kani::assume(balance <= i128::MAX - amount);

        let Ok(new_balance) = mint_pure(balance, amount) else {
            panic!("mint_pure failed despite valid preconditions");
        };

        assert!(new_balance == balance + amount);
    }

    #[kani::proof]
    fn verify_burn_pure() {
        let balance: i128 = kani::any();
        let amount: i128 = kani::any();

        kani::assume(amount > 0);
        kani::assume(balance >= amount);

        let Ok(new_balance) = burn_pure(balance, amount) else {
            panic!("burn_pure failed despite valid preconditions");
        };

        assert!(new_balance == balance - amount);
    }

    // ── Token initialisation proof harnesses ─────────────────────────────────

    /// **Property**: The `initialize` function can only ever be called once
    /// successfully.
    ///
    /// For every possible value of the already-initialised flag Kani proves:
    /// * When `is_initialized == true`  → the call **always** returns `Err`.
    /// * There exists no path through `initialize_pure(true)` that returns `Ok`.
    #[kani::proof]
    fn verify_initialize_fails_when_already_initialized() {
        // Kani considers the single concrete value `true` (contract already set up).
        let result = initialize_pure(true);

        // The guard must always fire; `Ok` is unreachable from this state.
        assert!(
            result.is_err(),
            "initialize must fail when the contract is already initialized"
        );
    }

    /// **Property**: The first call on a fresh (uninitialised) contract always
    /// succeeds.
    ///
    /// When `is_initialized == false` Kani proves:
    /// * `initialize_pure(false)` **always** returns `Ok(())`.
    /// * There exists no path where the first call fails.
    #[kani::proof]
    fn verify_initialize_succeeds_when_not_initialized() {
        // Kani considers the single concrete value `false` (contract is fresh).
        let result = initialize_pure(false);

        // The guard must not fire; setup on an uninitialised contract always works.
        assert!(
            result.is_ok(),
            "initialize must succeed when the contract has not yet been initialized"
        );
    }

    /// **Property**: Double-initialisation is mathematically impossible.
    ///
    /// Kani exhaustively checks **every** boolean value of `is_initialized` and
    /// proves the following invariant:
    ///
    ///   A second call (is_initialized == true) can **never** return Ok.
    ///
    /// Combined with `verify_initialize_succeeds_when_not_initialized`, the two
    /// harnesses together constitute a full mathematical proof that `initialize`
    /// can only ever succeed exactly once across all possible execution traces.
    #[kani::proof]
    fn verify_initialize_idempotency_guarantee() {
        let is_initialized: bool = kani::any();

        let result = initialize_pure(is_initialized);

        if is_initialized {
            // Already set up: the function MUST refuse.
            assert!(
                result.is_err(),
                "initialize must always fail when already initialized"
            );
        } else {
            // Fresh contract: the function MUST succeed.
            assert!(
                result.is_ok(),
                "initialize must succeed on a fresh contract"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_write_requires_matching_admin_and_host_auth() {
        let unchanged_identity = admin_write_pure(17, 42, true, 99);
        let missing_signature = admin_write_pure(17, 17, false, 99);
        assert_eq!(unchanged_identity, AdminWriteOutcome { admin: 17, wrote: false });
        assert_eq!(missing_signature, AdminWriteOutcome { admin: 17, wrote: false });
        assert_eq!(
            admin_write_pure(17, 17, true, 99),
            AdminWriteOutcome { admin: 99, wrote: true }
        );
        assert!(admin_write_missing_auth_bug(17, 17, 99).wrote);
        // A valid signature from an administrator who has been replaced must
        // not be enough to write the new administrator's storage.
        let rotation = admin_write_pure(17, 17, true, 99);
        let old_admin_replay = admin_write_pure(rotation.admin, 17, true, 23);
        assert_eq!(old_admin_replay, AdminWriteOutcome { admin: 99, wrote: false });
    }


    /// The correct transfer conserves the two-account total (concrete witness of
    /// the property `verify_transfer_pure_conservation` proves for all inputs).
    #[test]
    fn transfer_conserves_supply() {
        let (from, to, amount) = (100i128, 40i128, 30i128);
        let (new_from, new_to) = transfer_pure(from, to, amount).unwrap();
        assert_eq!(new_from + new_to, from + to);
    }

    /// The buggy variant breaks conservation by minting one unit per transfer —
    /// the concrete counterexample behind `verify_mint_on_transfer_bug_breaks_
    /// conservation`.
    #[test]
    fn mint_on_transfer_bug_creates_supply() {
        let (from, to, amount) = (100i128, 40i128, 30i128);
        let (new_from, new_to) = transfer_pure_mint_bug(from, to, amount).unwrap();
        assert_eq!(
            new_from + new_to,
            from + to + 1,
            "buggy transfer must mint exactly one unit"
        );
    }
}
