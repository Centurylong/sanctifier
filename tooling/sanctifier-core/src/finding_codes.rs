use serde::Serialize;

pub const AUTH_GAP: &str = "S001";
pub const PANIC_USAGE: &str = "S002";
pub const ARITHMETIC_OVERFLOW: &str = "S003";
pub const LEDGER_SIZE_RISK: &str = "S004";
pub const STORAGE_COLLISION: &str = "S005";
pub const UNSAFE_PATTERN: &str = "S006";
pub const CUSTOM_RULE_MATCH: &str = "S007";
pub const EVENT_INCONSISTENCY: &str = "S008";
pub const UNHANDLED_RESULT: &str = "S009";
pub const UPGRADE_RISK: &str = "S010";
pub const SMT_INVARIANT_VIOLATION: &str = "S011";
pub const HARDCODED_ADDR: &str = "S012";
pub const EDGE_AMOUNT: &str = "S013";
pub const DEPRECATED_SDK: &str = "S014";
pub const DEAD_CODE: &str = "S015";
pub const ERROR_CODE_COLLISION: &str = "S016";
pub const UNBOUND_AUTH: &str = "S024";
pub const FEE_ROUNDING: &str = "S017";
pub const UNSIGNED_UNDERFLOW: &str = "S019";
pub const LEDGER_SECONDS: &str = "S021";
pub const EXCESSIVE_CLONE: &str = "S020";
pub const ARG_DOS: &str = "SANCT_ARG_DOS";
pub const BALANCE_EQUALITY: &str = "SANCT_BALANCE_EQ";
pub const BLS_SUBGROUP_UNCHECKED: &str = "SANCT_BLS_SUBGROUP_UNCHECKED";
pub const SANCT_UNWRAP: &str = "SANCT_UNWRAP";
pub const INIT_HARDCODED_ADMIN: &str = "SANCT_INIT_HARDCODED_ADMIN";
pub const SANCT_VISIBILITY: &str = "SANCT_VISIBILITY";
pub const UNBOUNDED_STORAGE: &str = "SANCT_UNBOUNDED_STORAGE";
pub const SANCT_VIEW_PANIC: &str = "SANCT_VIEW_PANIC";
pub const ALLOWANCE_RACE: &str = "SANCT_ALLOWANCE_RACE";
pub const STATE_WRITE_IN_VIEW: &str = "SANCT_STATE_WRITE_IN_VIEW";
pub const DIVISION_BY_ZERO: &str = "S018";
pub const TIER_BOUNDARY_OFF_BY_ONE: &str = "S022";
pub const MISSING_RESERVE_AUTH: &str = "S023";
pub const CROSS_CONTRACT_CALL_IN_LOOP: &str = "SANCT_CROSS_CONTRACT_CALL_IN_LOOP";
pub const UNBOUNDED_EVENT_EMISSION: &str = "SANCT_UNBOUNDED_EVENT_EMISSION";
pub const SEP41_ALLOWANCE_NOT_DECREMENTED: &str = "SANCT_SEP41_ALLOWANCE_NOT_DECREMENTED";
pub const SEP41_APPROVAL_NO_EXPIRATION: &str = "SANCT_SEP41_APPROVAL_NO_EXPIRATION";
pub const NULLIFIER_GROWTH: &str = "SANCT_NULLIFIER_GROWTH";
pub const PROOF_LENGTH_UNVALIDATED: &str = "SANCT_PROOF_LENGTH_UNVALIDATED";
pub const PUBLIC_INPUT_UNVALIDATED: &str = "SANCT_PUBLIC_INPUT_UNVALIDATED";
pub const AUTH_REPLAY: &str = "SANCT_AUTH_REPLAY";

// ── Source-optional (compiled WASM) checks ────────────────────────────────────
// Emitted only by `sanctifier wasm`, which analyzes a deployed module directly.
pub const WASM_NOT_SOROBAN: &str = "W001";
pub const WASM_NO_EXPORTS: &str = "W002";
pub const WASM_MISSING_ENV_META: &str = "W003";
pub const WASM_FLOAT_TYPES: &str = "W004";

#[derive(Debug, Clone, Serialize)]
pub struct FindingCode {
    pub code: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

pub fn all_finding_codes() -> Vec<FindingCode> {
    vec![
        FindingCode {
            code: AUTH_GAP,
            category: "authentication",
            description: "Missing authentication guard in a state-mutating function",
        },
        FindingCode {
            code: PANIC_USAGE,
            category: "panic_handling",
            description: "panic!/unwrap/expect usage that may cause runtime aborts",
        },
        FindingCode {
            code: ARITHMETIC_OVERFLOW,
            category: "arithmetic",
            description: "Unchecked arithmetic operation with overflow/underflow risk",
        },
        FindingCode {
            code: LEDGER_SIZE_RISK,
            category: "storage_limits",
            description: "Ledger entry size is exceeding or approaching configured threshold",
        },
        FindingCode {
            code: STORAGE_COLLISION,
            category: "storage_keys",
            description: "Potential storage key collision across contract data paths",
        },
        FindingCode {
            code: UNSAFE_PATTERN,
            category: "unsafe_patterns",
            description: "Potentially unsafe language/runtime pattern was detected",
        },
        FindingCode {
            code: CUSTOM_RULE_MATCH,
            category: "custom_rule",
            description: "User-defined rule matched contract source",
        },
        FindingCode {
            code: EVENT_INCONSISTENCY,
            category: "events",
            description: "Inconsistent topic counts or sub-optimal gas patterns in events",
        },
        FindingCode {
            code: UNHANDLED_RESULT,
            category: "logic",
            description: "A function call returns a Result that is not consumed or handled",
        },
        FindingCode {
            code: UPGRADE_RISK,
            category: "upgrades",
            description: "Potential security risk in contract upgrade or admin mechanisms",
        },
        FindingCode {
            code: SMT_INVARIANT_VIOLATION,
            category: "formal_verification",
            description: "Formal verification (Z3) proved a mathematical violation of an invariant",
        },
        FindingCode {
            code: HARDCODED_ADDR,
            category: "code_hygiene",
            description: "Hardcoded admin address or secret literal used in authentication context",
        },
        FindingCode {
            code: EDGE_AMOUNT,
            category: "code_hygiene",
            description: "Transfer/mint/burn missing amount>0 or from!=to validation guards",
        },
        FindingCode {
            code: BALANCE_EQUALITY,
            category: "logic",
            description:
                "Balance gated against an amount with `==`/`!=` where `>=`/`<=` was likely intended",
        },
        FindingCode {
            code: BLS_SUBGROUP_UNCHECKED,
            category: "cryptography",
            description:
                "BLS12-381 proof point reaches a pairing call with no subgroup or curve membership check",
        },
        FindingCode {
            code: DEPRECATED_SDK,
            category: "code_hygiene",
            description:
                "Deprecated soroban-sdk host function with suggested replacement available",
        },
        FindingCode {
            code: DEAD_CODE,
            category: "code_hygiene",
            description: "Dead code or always-true guard condition detected via constant folding",
        },
        FindingCode {
            code: ERROR_CODE_COLLISION,
            category: "code_hygiene",
            description: "Inconsistent or duplicate discriminants in #[contracterror] enum",
        },
        FindingCode {
            code: UNBOUND_AUTH,
            category: "authentication",
            description: "Internal function uses require_auth() instead of require_auth_for_args()",
        },
        FindingCode {
            code: FEE_ROUNDING,
            category: "arithmetic",
            description:
                "Fee/interest calculation using integer division rounds to zero for micro-amounts, enabling fee-evasion attacks",
        },
        FindingCode {
            code: UNSIGNED_UNDERFLOW,
            category: "arithmetic",
            description:
                "Unchecked subtraction on an unsigned integer can wrap past zero (underflow)",
        },
        FindingCode {
            code: LEDGER_SECONDS,
            category: "time_logic",
            description:
                "Ledger sequence number (block counter) mixed with a seconds-magnitude literal; use timestamp() for real-time windows",
        },
        FindingCode {
            code: EXCESSIVE_CLONE,
            category: "gas_efficiency",
            description:
                "Gas-wasting clone of the Soroban Env handle where a reference (&env) would suffice",
        },
        FindingCode {
            code: ARG_DOS,
            category: "denial_of_service",
            description:
                "Contract entrypoint iterates over a Vec or Map argument without a visible length cap",
        },
        FindingCode {
            code: SANCT_UNWRAP,
            category: "panic_handling",
            description:
                "Contract entrypoint uses unwrap, expect, or a risky unwrap_or_default fallback",
        },
        FindingCode {
            code: INIT_HARDCODED_ADMIN,
            category: "authentication",
            description:
                "Initialization function uses hardcoded admin address literal or default value instead of formal argument",
        },
        FindingCode {
            code: SANCT_VISIBILITY,
            category: "authentication",
            description: "Helper-shaped state mutator is publicly exposed without authorization",
        },
        FindingCode {
            code: UNBOUNDED_STORAGE,
            category: "denial_of_service",
            description:
                "Persistent/instance storage collection grows via append/insert with no removal or length cap",
        },
        FindingCode {
            code: SANCT_VIEW_PANIC,
            category: "panic_handling",
            description:
                "View/getter entrypoint contains a reachable panic, aborting callers that assume reads are safe",
        },
        FindingCode {
            code: ALLOWANCE_RACE,
            category: "authorization",
            description:
                "Allowance is overwritten unconditionally (set-allowance) without increase/decrease or compare-and-set semantics, enabling the approve front-running race",
        },
        FindingCode {
            code: STATE_WRITE_IN_VIEW,
            category: "code_hygiene",
            description:
                "Getter/view-style function performs a storage write; callers expect it to be read-only",
        },
        FindingCode {
            code: DIVISION_BY_ZERO,
            category: "arithmetic",
            description:
                "Division or modulo by a non-constant value not proven non-zero, which panics on-chain if zero at runtime",
        },
        FindingCode {
            code: TIER_BOUNDARY_OFF_BY_ONE,
            category: "logic",
            description:
                "if/else-if boundary ladder mixes strict (<, >) and inclusive (<=, >=) comparisons against the same variable, a common source of off-by-one tier/rank misassignment",
        },
        FindingCode {
            code: MISSING_RESERVE_AUTH,
            category: "authorization",
            description: "Missing strict authorization guard on reserve or treasury funds withdrawal",
        },
        FindingCode {
            code: CROSS_CONTRACT_CALL_IN_LOOP,
            category: "resource_limits",
            description: "Cross-contract call (invoke_contract or a *Client) issued from inside a loop",
        },
        FindingCode {
            code: UNBOUNDED_EVENT_EMISSION,
            category: "resource_limits",
            description: "env.events().publish(..) issued from inside a loop with no iteration bound",
        },
        FindingCode {
            code: SEP41_ALLOWANCE_NOT_DECREMENTED,
            category: "authorization",
            description: "transfer_from reads the caller's allowance but never writes back a decremented value",
        },
        FindingCode {
            code: SEP41_APPROVAL_NO_EXPIRATION,
            category: "authorization",
            description: "approve has no expiration_ledger parameter, or never uses the one it accepts",
        },
        FindingCode {
            code: NULLIFIER_GROWTH,
            category: "denial_of_service",
            description:
                "ZK nullifier/commitment marked spent in persistent storage with no pruning, TTL extension, or bounded-size check",
        },
        FindingCode {
            code: PROOF_LENGTH_UNVALIDATED,
            category: "zk_verification",
            description:
                "Proof or public-input byte array reaches a verifier call without a length check first",
        },
        FindingCode {
            code: PUBLIC_INPUT_UNVALIDATED,
            category: "zk_verification",
            description:
                "Proof verification consumes public inputs without checking they are canonical field elements in range",
        },
        FindingCode {
            code: WASM_NOT_SOROBAN,
            category: "wasm",
            description:
                "Compiled module has no Soroban contract spec section; may not be a Soroban contract",
        },
        FindingCode {
            code: WASM_NO_EXPORTS,
            category: "wasm",
            description: "Compiled module exports no callable functions",
        },
        FindingCode {
            code: WASM_MISSING_ENV_META,
            category: "wasm",
            description:
                "Compiled module is missing Soroban environment metadata (interface version)",
        },
        FindingCode {
            code: WASM_FLOAT_TYPES,
            category: "wasm",
            description:
                "Compiled module uses floating-point value types, which the Soroban host rejects",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn finding_codes_are_unique() {
        let codes = all_finding_codes();
        let unique: HashSet<&str> = codes.iter().map(|c| c.code).collect();
        assert_eq!(codes.len(), unique.len());
    }

    #[test]
    fn includes_expected_codes() {
        let codes = all_finding_codes();
        assert!(codes.iter().any(|c| c.code == AUTH_GAP));
        assert!(codes.iter().any(|c| c.code == PANIC_USAGE));
        assert!(codes.iter().any(|c| c.code == ARITHMETIC_OVERFLOW));
        assert!(codes.iter().any(|c| c.code == LEDGER_SIZE_RISK));
        assert!(codes.iter().any(|c| c.code == STORAGE_COLLISION));
        assert!(codes.iter().any(|c| c.code == UNSAFE_PATTERN));
        assert!(codes.iter().any(|c| c.code == CUSTOM_RULE_MATCH));
        assert!(codes.iter().any(|c| c.code == SANCT_UNWRAP));
        assert!(codes.iter().any(|c| c.code == SANCT_VISIBILITY));
        assert!(codes.iter().any(|c| c.code == INIT_HARDCODED_ADMIN));
        assert!(codes.iter().any(|c| c.code == UNBOUNDED_STORAGE));
        assert!(codes.iter().any(|c| c.code == SANCT_VIEW_PANIC));
        assert!(codes.iter().any(|c| c.code == ALLOWANCE_RACE));
        assert!(codes.iter().any(|c| c.code == DIVISION_BY_ZERO));
        assert!(codes.iter().any(|c| c.code == MISSING_RESERVE_AUTH));
    }
}
