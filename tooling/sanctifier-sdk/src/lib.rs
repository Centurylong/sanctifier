#![no_std]

/// Unified invariant guard that bridges runtime safety and formal verification.
/// 
/// - In standard runtime/tests: expands to `assert!`.
/// - In Kani formal verification: expands to `kani::assert!`.
/// - In Sanctifier static analysis: flagged as a logical constraint.
/// - Environment-Aware mode: when `Env` is provided, publishes `SanctityViolation` event before panicking.
#[macro_export]
macro_rules! guard_invariant {
    // -------------------------------------------------------------------------
    // Case 1: env, condition, message
    ($env:expr, $cond:expr, $msg:expr) => {
        #[cfg(kani)]
        kani::assert!($cond, $msg);

        #[cfg(not(kani))]
        if !$cond {
            #[cfg(feature = "soroban")]
            {
                // Emit event to Host so indexers can track the exact violation
                $env.events().publish(
                    (soroban_sdk::symbol_short!("sanctity"), soroban_sdk::symbol_short!("violation")),
                    $msg
                );
            }
            panic!("Sanctity violation at {}:{}: {}", file!(), line!(), $msg);
        }
    };

    // -------------------------------------------------------------------------
    // Case 2: condition, message
    ($cond:expr, $msg:expr) => {
        #[cfg(kani)]
        kani::assert!($cond, $msg);

        #[cfg(not(kani))]
        assert!($cond, "Sanctity violation at {}:{}: {}", file!(), line!(), $msg);
    };

    // -------------------------------------------------------------------------
    // Case 3: condition
    ($cond:expr) => {
        #[cfg(kani)]
        kani::assert!($cond, "Invariant guard failed");

        #[cfg(not(kani))]
        assert!($cond, "Sanctity violation at {}:{}: Invariant guard failed", file!(), line!());
    };
}
