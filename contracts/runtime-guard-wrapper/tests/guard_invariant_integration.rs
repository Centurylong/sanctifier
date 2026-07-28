#![cfg(test)]

// Integration tests for the guard_invariant! macro wiring inside the
// runtime-guard-wrapper. These verify the end-to-end behaviour the issue
// asks for: when an invariant breaks at runtime, the wrapper publishes
// an `inv_fail` event (the on-chain audit trail) AND surfaces the typed
// error to the caller.
//
// The wrapper uses `guard_invariant_result!` rather than the trapping
// `guard_invariant!`, so its outer `Result<Val, Error>` signature can
// bubble the typed error to the host through the normal Soroban Result
// path. This sidesteps the soroban-sdk 20.5 testutils limitation around
// catching `panic_with_error!` and keeps the event-publishing semantics
// observable end to end.

use runtime_guard_wrapper::{GuardError, RuntimeGuardWrapper, RuntimeGuardWrapperClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events},
    Address, Env, Error, IntoVal, Symbol, TryFromVal, Val, Vec,
};

fn setup() -> (Env, Address) {
    let env = Env::default();
    let id = env.register_contract(None, RuntimeGuardWrapper);
    (env, id)
}

fn topic_is(env: &Env, topics: &Vec<Val>, expected: &str) -> bool {
    if topics.is_empty() {
        return false;
    }
    let first = topics.first().unwrap();
    match Symbol::try_from_val(env, &first) {
        Ok(sym) => sym == Symbol::new(env, expected),
        Err(_) => false,
    }
}

fn count_topic(env: &Env, expected: &str) -> usize {
    env.events()
        .all()
        .iter()
        .filter(|(_addr, topics, _data)| topic_is(env, topics, expected))
        .count()
}

#[test]
fn pre_exec_guard_publishes_inv_fail_when_wrapped_contract_missing() {
    let (env, id) = setup();
    let client = RuntimeGuardWrapperClient::new(&env, &id);

    // No `init` was called, so the wrapped contract address is missing.
    // The pre_execution_guards site should publish an `inv_fail` audit
    // event and return GuardError::WrappedContractMissing.
    let fn_name: Symbol = symbol_short!("noop");
    let args: Vec<Val> = Vec::new(&env);
    let result = client.try_execute_guarded(&fn_name, &args);

    assert!(
        result.is_err(),
        "execute_guarded should reject when no wrapped contract is set"
    );
    let surfaced: Error = result.unwrap_err().unwrap();
    let expected: Error = GuardError::WrappedContractMissing.into();
    assert_eq!(surfaced, expected, "wrong typed error code");

    assert!(
        count_topic(&env, "inv_fail") >= 1,
        "no inv_fail audit event was published; the audit trail is broken"
    );
}

#[test]
fn init_then_execute_guarded_does_not_emit_inv_fail() {
    let (env, id) = setup();
    let client = RuntimeGuardWrapperClient::new(&env, &id);

    // Configure a wrapped target so the pre and post invariant checks pass.
    let wrapped = Address::generate(&env);
    client.init(&wrapped);

    let fn_name: Symbol = symbol_short!("noop");
    let args: Vec<Val> = Vec::new(&env);
    let result = client.try_execute_guarded(&fn_name, &args);

    assert!(
        result.is_ok(),
        "execute_guarded should succeed after init: {:?}",
        result
    );

    assert_eq!(
        count_topic(&env, "inv_fail"),
        0,
        "happy path emitted an inv_fail audit event"
    );
}

#[test]
fn inv_fail_payload_carries_condition_source_string() {
    // The macro records `stringify!($cond)` as the data payload. Off-chain
    // indexers rely on this so the same event topic can report many
    // different invariant violations across contracts. Verify the payload
    // is a non-empty string-shaped Val on the failing path.
    let (env, id) = setup();
    let client = RuntimeGuardWrapperClient::new(&env, &id);

    let fn_name: Symbol = symbol_short!("noop");
    let args: Vec<Val> = Vec::new(&env);
    let _ = client.try_execute_guarded(&fn_name, &args);

    let mut found_payload = false;
    for (_addr, topics, data) in env.events().all().iter() {
        if !topic_is(&env, &topics, "inv_fail") {
            continue;
        }
        // The data tuple is (symbol_short!("cond"), String::from_str(env, "...")).
        // Soroban events serialise tuples as a Vec<Val>; the second element
        // should round-trip into a soroban_sdk::String.
        let data_vec_result: Result<Vec<Val>, _> = Vec::try_from_val(&env, &data);
        if let Ok(data_vec) = data_vec_result {
            if data_vec.len() >= 2 {
                let payload_val = data_vec.get(1).unwrap();
                if soroban_sdk::String::try_from_val(&env, &payload_val).is_ok() {
                    found_payload = true;
                }
            }
        }
    }
    assert!(
        found_payload,
        "inv_fail event was published but its condition payload is missing or wrong shape"
    );
}

#[test]
fn health_check_after_init_returns_true() {
    // Regression sanity: wiring the macro must not have broken the
    // existing health_check entry. health_check reads the same storage
    // keys the new guard sites assert on.
    let (env, id) = setup();
    let client = RuntimeGuardWrapperClient::new(&env, &id);

    let wrapped = Address::generate(&env);
    client.init(&wrapped);

    // execute_guarded once so EXECUTION_METRICS gets populated, which is
    // what health_check verifies in addition to the wrapped address.
    let fn_name: Symbol = symbol_short!("noop");
    let args: Vec<Val> = Vec::new(&env);
    let _ = client.execute_guarded(&fn_name, &args);

    assert!(client.health_check());
}

#[allow(dead_code)]
fn _suppress_unused<V: IntoVal<Env, Val>>(_: V) {}
