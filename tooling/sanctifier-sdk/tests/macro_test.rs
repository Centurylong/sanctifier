use sanctifier_sdk::guard_invariant;

#[test]
fn test_basic_guard() {
    guard_invariant!(true);
}

#[test]
#[should_panic(expected = "Sanctity violation")]
fn test_basic_guard_panic() {
    guard_invariant!(false);
}

#[test]
fn test_msg_guard() {
    guard_invariant!(true, "Should not panic");
}

#[test]
#[should_panic(expected = "Custom error")]
fn test_msg_guard_panic() {
    guard_invariant!(false, "Custom error");
}

#[cfg(feature = "soroban")]
#[test]
#[should_panic(expected = "Sanctity violation")]
fn test_env_guard_panic() {
    use soroban_sdk::Env;
    let env = Env::default();
    guard_invariant!(env, false, "Env-aware panic");
}
