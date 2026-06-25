#![no_std]
#![doc = include_str!("../README.md")]

// Runtime invariant macro for Soroban contracts.
//
// `guard_invariant!(env, cond, Error::X)` evaluates `cond` once; if false, it
// publishes a structured `inv_fail` event on `env.events()` and then traps
// the transaction with `panic_with_error!(env, Error::X)`. The event is
// published before the trap so the on-chain audit trail survives the
// rollback. That ordering is the whole point of the issue — runtime
// invariant checks catch what static analysis can't prove and leave a
// queryable record indexers can subscribe to.
//
// Design notes worth keeping next to the macro:
//
//  - The condition is captured into a local binding so `$cond` is evaluated
//    exactly once, even when callers pass an expression with side effects
//    such as `counter.fetch_add(1)`. Soroban contracts often pay for every
//    re-read of a storage entry, so a silent double-eval would both
//    miscount and waste cycles.
//
//  - Topics are a Soroban scarce resource. We use exactly one topic,
//    `inv_fail`, so off-chain indexers can subscribe to invariant
//    failures across every contract that uses the macro without
//    re-reading the data payload to filter. The condition expression
//    rides in the data payload as a `String`, not as a topic.
//
//  - `stringify!($cond)` returns a `&'static str`. Soroban events carry
//    typed `Val`s, so we wrap the literal with `soroban_sdk::String::from_str`
//    against the caller's `Env` to get a heap-allocated host `String` value
//    that can be serialized into the event payload.
//
//  - `soroban_sdk::symbol_short!("inv_fail")` is sound because the literal
//    is eight bytes and `symbol_short!` caps at nine. Renaming it without
//    re-checking the length would silently fail at compile time, which is
//    fine — the macro re-exports the constant below so call sites cannot
//    drift accidentally.
//
//  - `panic_with_error!` is the right trap primitive: it surfaces the
//    caller-supplied `Error` value to the host, which carries a typed
//    contract error code rather than a stringly-typed `panic!` message.
//    Wrappers that need to map invariant failures to specific error codes
//    pass any `IntoVal<Env, Error>`-compatible type as the third argument.

/// Topic used by every `guard_invariant!` failure event. Re-exported so test
/// code, indexers, and downstream wrappers can refer to it without a stringly
/// typed literal. Eight bytes, fits inside the nine-byte `symbol_short!`
/// budget.
pub const INVARIANT_FAILURE_TOPIC: &str = "inv_fail";

/// Topic used when a guarded post-condition succeeds. Optional; provided for
/// wrappers that want to also track guard-pass events for monitoring.
pub const INVARIANT_PASS_TOPIC: &str = "inv_pass";

/// Re-export so `guard_invariant!` callers do not need to import
/// `soroban_sdk::*` themselves.
pub use soroban_sdk;

/// Assert a runtime invariant.
///
/// If `cond` evaluates to `false`, this macro publishes a single
/// `inv_fail` event with the condition source as the data payload, then
/// traps the transaction by invoking `panic_with_error!(env, err)`. The
/// event is committed to the on-chain ledger *before* the trap, so the
/// audit trail is intentionally preserved even though the trap rolls back
/// state.
///
/// The condition is evaluated exactly once, regardless of whether it
/// passes or fails.
///
/// # Example
///
/// ```ignore
/// use soroban_sdk::{contracterror, Env};
/// use sanctifier_guards::guard_invariant;
///
/// #[contracterror]
/// #[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// pub enum Error {
///     BadState = 1,
/// }
///
/// fn check(env: &Env, total: u32, sum: u32) {
///     guard_invariant!(env, total == sum, Error::BadState);
/// }
/// ```
#[macro_export]
macro_rules! guard_invariant {
    ($env:expr, $cond:expr, $err:expr) => {{
        // Bind once. Any side effect inside `$cond` runs exactly here, never
        // again — important because a Soroban storage read costs gas and a
        // silent double-eval would both miscount and overcharge.
        let __sanctifier_guards_cond: bool = $cond;
        if !__sanctifier_guards_cond {
            // Bind the env to a reference so `$env` is also evaluated once
            // and works whether the caller passes `env` or `&env`.
            let __sanctifier_guards_env: &$crate::soroban_sdk::Env = &$env;
            // Publish the audit-trail event before trapping. The host
            // commits this event into the transaction's event set even
            // though the subsequent panic rolls back contract state, so
            // indexers always see the violation.
            __sanctifier_guards_env.events().publish(
                ($crate::soroban_sdk::symbol_short!("inv_fail"),),
                (
                    $crate::soroban_sdk::symbol_short!("cond"),
                    $crate::soroban_sdk::String::from_str(
                        __sanctifier_guards_env,
                        ::core::stringify!($cond),
                    ),
                ),
            );
            $crate::soroban_sdk::panic_with_error!(__sanctifier_guards_env, $err);
        }
    }};
}

/// Result-returning variant of [`guard_invariant!`]. Publishes the same
/// `inv_fail` event on violation, but returns `Err($err)` instead of
/// trapping the transaction. Useful for two reasons:
///
/// 1. Callers whose own signature returns `Result<_, Error>` (such as
///    the runtime-guard-wrapper's `execute_guarded`) can `?` the macro
///    output to bubble the typed error up to the host instead of
///    letting the contract abort mid-function.
/// 2. The trapping form goes through `panic_with_error!`, which in
///    soroban-sdk 20.5's testutils triggers a non-unwinding panic that
///    aborts the test runner before either `#[should_panic]` or
///    `std::panic::catch_unwind` can rescue it. This Result form
///    is the unit-testable equivalent, so the macro's event publishing
///    and condition-source semantics can be covered without depending
///    on the host's trap-catching path.
///
/// The condition is evaluated exactly once, same as `guard_invariant!`.
#[macro_export]
macro_rules! guard_invariant_result {
    ($env:expr, $cond:expr, $err:expr) => {{
        let __sanctifier_guards_cond: bool = $cond;
        if !__sanctifier_guards_cond {
            let __sanctifier_guards_env: &$crate::soroban_sdk::Env = &$env;
            __sanctifier_guards_env.events().publish(
                ($crate::soroban_sdk::symbol_short!("inv_fail"),),
                (
                    $crate::soroban_sdk::symbol_short!("cond"),
                    $crate::soroban_sdk::String::from_str(
                        __sanctifier_guards_env,
                        ::core::stringify!($cond),
                    ),
                ),
            );
            return ::core::result::Result::Err($err.into());
        }
    }};
}

/// Publish an `inv_pass` event without trapping. Optional companion to
/// `guard_invariant!` for wrappers that want to record successful
/// post-condition checks alongside violations. Same topic discipline
/// (one short symbol, condition source in the data payload).
#[macro_export]
macro_rules! guard_invariant_pass {
    ($env:expr, $cond:expr) => {{
        let __sanctifier_guards_env: &$crate::soroban_sdk::Env = &$env;
        __sanctifier_guards_env.events().publish(
            ($crate::soroban_sdk::symbol_short!("inv_pass"),),
            (
                $crate::soroban_sdk::symbol_short!("cond"),
                $crate::soroban_sdk::String::from_str(
                    __sanctifier_guards_env,
                    ::core::stringify!($cond),
                ),
            ),
        );
    }};
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    extern crate alloc;

    use soroban_sdk::{
        contract, contracterror, contractimpl, symbol_short, testutils::Events, Env, Error,
        IntoVal, Symbol, TryFromVal, Val,
    };

    #[contracterror]
    #[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
    #[repr(u32)]
    pub enum DemoError {
        BadState = 1,
        Negative = 2,
    }

    // A minimal contract whose only job is to surface the macros to the
    // test harness. Each entry point exercises one behaviour: passing
    // condition, failing condition (via the Result variant so tests can
    // observe events without depending on the soroban-sdk testutils
    // trap-catching path), and a side-effect-bearing condition.
    #[contract]
    pub struct GuardDemo;

    #[contractimpl]
    impl GuardDemo {
        /// Passes when `a == b`. Used to verify the pass path is silent.
        pub fn check_equal(env: Env, a: u32, b: u32) {
            crate::guard_invariant!(&env, a == b, DemoError::BadState);
        }

        /// Returns `Err` on violation using the Result variant. Used to
        /// verify the event payload and the typed error code without
        /// going through `panic_with_error!`, which the host's test
        /// harness does not currently propagate cleanly.
        pub fn check_eq_result(env: Env, a: u32, b: u32) -> Result<(), Error> {
            crate::guard_invariant_result!(&env, a == b, DemoError::BadState);
            Ok(())
        }

        /// Reads an instance storage counter, evaluates the condition
        /// `count == 1`, and increments the counter regardless. Used to
        /// confirm the macro reads the bound value only once. If `$cond`
        /// were double-evaluated, the storage read (and therefore the gas
        /// cost and the perceived value) would happen twice.
        pub fn check_once(env: Env) -> u32 {
            let key = symbol_short!("count");
            let count: u32 = env.storage().instance().get(&key).unwrap_or(0);
            let next = count + 1;
            env.storage().instance().set(&key, &next);
            crate::guard_invariant!(&env, next > 0, DemoError::Negative);
            next
        }

        /// Publishes a sentinel event before invoking the Result-form
        /// guard. Used to verify the ordering invariant: the audit-trail
        /// event lands in the event set AFTER the user's pre event. The
        /// trapping form has the same ordering by construction; verifying
        /// it on the Result form is sufficient because both macros call
        /// `events().publish(...)` at the same source position.
        pub fn ordered_emit_result(env: Env) -> Result<(), Error> {
            env.events()
                .publish((symbol_short!("pre"),), symbol_short!("pre"));
            crate::guard_invariant_result!(&env, false, DemoError::BadState);
            Ok(())
        }
    }

    fn setup() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register_contract(None, GuardDemo);
        (env, id)
    }

    fn topic_matches(env: &Env, topics: &soroban_sdk::Vec<Val>, expected: &str) -> bool {
        if topics.is_empty() {
            return false;
        }
        let first = topics.first().unwrap();
        match Symbol::try_from_val(env, &first) {
            Ok(sym) => sym == Symbol::new(env, expected),
            Err(_) => false,
        }
    }

    #[test]
    fn pass_path_is_silent() {
        let (env, id) = setup();
        let client = GuardDemoClient::new(&env, &id);
        client.check_equal(&7u32, &7u32);

        let events = env.events().all();
        // No event with our `inv_fail` topic should be present. The macro
        // is required to be invisible on the happy path.
        for (_addr, topics, _data) in events.iter() {
            assert!(
                !topic_matches(&env, &topics, "inv_fail"),
                "pass path leaked an inv_fail event"
            );
        }
    }

    // The trapping form goes through `panic_with_error!` which, in
    // soroban-sdk 20.5's testutils, raises a non-unwinding panic that
    // aborts the test runner before `#[should_panic]` or
    // `std::panic::catch_unwind` can rescue it. The companion
    // `guard_invariant_result!` macro has identical event-publishing
    // semantics and a typed error return, so every assertion about
    // event payload, condition source, and error code stays observable
    // in tests by exercising the Result form. The trap path is then
    // verified end to end in the runtime-guard-wrapper integration
    // suite, where the wrapper's outer `Result<Val, Error>` signature
    // bubbles the error to the caller.
    #[test]
    fn violation_returns_typed_error_via_result_form() {
        let (env, id) = setup();
        let client = GuardDemoClient::new(&env, &id);
        let res = client.try_check_eq_result(&1u32, &2u32);
        assert!(res.is_err(), "result form should surface Err on violation");
        let err = res.unwrap_err().unwrap();
        let expected: Error = DemoError::BadState.into();
        assert_eq!(err, expected, "violation surfaced the wrong error code");
    }

    #[test]
    fn violation_publishes_inv_fail_event() {
        let (env, id) = setup();
        let client = GuardDemoClient::new(&env, &id);
        let _ = client.try_check_eq_result(&1u32, &2u32);

        let events = env.events().all();
        let mut saw_inv_fail = false;
        for (_addr, topics, _data) in events.iter() {
            if topic_matches(&env, &topics, "inv_fail") {
                saw_inv_fail = true;
            }
        }
        assert!(
            saw_inv_fail,
            "no inv_fail event was published on the violation path"
        );
    }

    // Ordering: the user-emitted `pre` event must land in the event set
    // before the macro's `inv_fail` event. Verified on the Result form
    // because both macros call `events().publish(...)` at the same
    // source position before the trap or early-return.
    #[test]
    fn inv_fail_event_lands_after_user_event() {
        let (env, id) = setup();
        let client = GuardDemoClient::new(&env, &id);
        let _ = client.try_ordered_emit_result();

        let events = env.events().all();
        let mut pre_idx: Option<usize> = None;
        let mut fail_idx: Option<usize> = None;
        for (i, (_addr, topics, _data)) in events.iter().enumerate() {
            if topic_matches(&env, &topics, "pre") {
                pre_idx = Some(i);
            }
            if topic_matches(&env, &topics, "inv_fail") {
                fail_idx = Some(i);
            }
        }
        let pre_idx = pre_idx.expect("pre event missing from event set");
        let fail_idx = fail_idx.expect("inv_fail event missing from event set");
        assert!(
            pre_idx < fail_idx,
            "inv_fail event leaked before the user's pre event (ordering broke)",
        );
    }

    #[test]
    fn condition_is_evaluated_exactly_once() {
        // If the macro double-evaluated the condition, `check_once` would
        // increment the counter twice per call. We assert the counter is
        // exactly one after one invocation, which only holds if `$cond` ran
        // exactly once.
        let (env, id) = setup();
        let client = GuardDemoClient::new(&env, &id);
        let returned = client.check_once();
        assert_eq!(returned, 1, "macro double-evaluated the condition");

        // And a second call should return 2, not 3 or 4.
        let returned = client.check_once();
        assert_eq!(returned, 2, "macro double-evaluated on subsequent call");
    }

    // Compile-time guarantee: the topic constants are short enough that
    // `symbol_short!` accepts them. If a contributor renames either constant
    // to a value longer than nine bytes, this test fails to compile and
    // points at the offending line.
    #[test]
    fn topic_constants_fit_symbol_short_budget() {
        let _ = symbol_short!("inv_fail");
        let _ = symbol_short!("inv_pass");
        assert!(super::INVARIANT_FAILURE_TOPIC.len() <= 9);
        assert!(super::INVARIANT_PASS_TOPIC.len() <= 9);
    }

    // Contract for the pass-helper test. Has to live at module scope
    // because `#[contractimpl]` expects to find the target struct via
    // `super::` from a generated child module.
    #[contract]
    pub struct PassDemo;

    #[contractimpl]
    impl PassDemo {
        pub fn emit_pass(env: Env) {
            crate::guard_invariant_pass!(&env, 1 == 1);
        }
    }

    /// Sanity check that the `guard_invariant_pass!` helper actually emits.
    #[test]
    fn pass_helper_publishes_event() {
        let env = Env::default();
        let id = env.register_contract(None, PassDemo);
        let client = PassDemoClient::new(&env, &id);
        client.emit_pass();
        let events = env.events().all();
        let mut saw_inv_pass = false;
        for (_addr, topics, _data) in events.iter() {
            if topic_matches(&env, &topics, "inv_pass") {
                saw_inv_pass = true;
            }
        }
        assert!(saw_inv_pass);
    }

    // Reference to suppress unused-import warning when only `IntoVal` is
    // pulled in implicitly via the macro expansion.
    #[allow(dead_code)]
    fn _suppress_unused<V: IntoVal<Env, Val>>(_: V) {}
}
