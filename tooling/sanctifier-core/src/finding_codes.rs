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
pub const FEE_ROUNDING: &str = "S017";
pub const ARG_DOS: &str = "SANCT_ARG_DOS";
pub const SANCT_UNWRAP: &str = "SANCT_UNWRAP";
pub const SHIFT_OVERFLOW: &str = "SANCT_SHIFT_OVERFLOW";

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
            code: FEE_ROUNDING,
            category: "arithmetic",
            description:
                "Fee/interest calculation using integer division rounds to zero for micro-amounts, enabling fee-evasion attacks",
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
            code: SHIFT_OVERFLOW,
            category: "arithmetic",
            description:
                "Bit shift operation uses an unbounded or out-of-range shift amount",
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
        assert!(codes.iter().any(|c| c.code == SHIFT_OVERFLOW));
    }
}
