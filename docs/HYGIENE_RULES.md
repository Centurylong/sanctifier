# Hygiene Rules Pack - Implementation Documentation

## Overview

This document describes the implementation of three code-hygiene detector rules delivered as part of issue #523. These rules share common AST-visitor infrastructure and follow the established detector pattern in Sanctifier.

## Implemented Rules

### 1. SANCT_HARDCODED_ADDR (S012) - Hardcoded Address/Secret Detection

**Finding Code:** `S012`  
**Category:** `code_hygiene`  
**Severity:** Error (for secrets), Warning (for addresses)

**Description:**  
Detects hardcoded admin addresses or secret literals in authentication contexts. This is a critical security anti-pattern that can lead to unauthorized access or key leakage.

**What it detects:**
- Stellar public addresses (G... format, 56 chars)
- Stellar secret keys (S... format, 56 chars) - **Critical**
- Hex-encoded addresses (64 hex characters)
- Byte arrays of suspicious lengths (32, 56, 64 bytes) in auth contexts
- String literals containing "secret", "SECRET", "admin", "ADMIN" over 20 chars

**Examples:**

❌ **Vulnerable Code:**
```rust
pub fn initialize(env: Env) {
    // VIOLATION: Hardcoded admin address
    let admin = "GDJKFGJFKJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJG";
    env.storage().instance().set(&"admin", &admin);
}

pub fn verify_auth(env: Env) {
    // VIOLATION: Hardcoded secret key (Critical!)
    let secret = "SDJKFGJFKJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJGKFJG";
}
```

✅ **Clean Code:**
```rust
pub fn initialize(env: Env, admin: Address) {
    // Pass as parameter
    admin.require_auth();
    env.storage().instance().set(&Symbol::new(&env, "admin"), &admin);
}
```

**Fix Suggestion:**  
Store sensitive values in contract storage or pass as parameters. Never hardcode secrets in source code.

---

### 2. SANCT_ERROR_CODES (S016) - Error Code Collision Detection

**Finding Code:** `S016`  
**Category:** `code_hygiene`  
**Severity:** Error (duplicates), Warning (inconsistent), Info (gaps)

**Description:**  
Detects inconsistent or duplicate discriminants in `#[contracterror]` enums. Duplicate error codes can cause runtime bugs and make debugging impossible.

**What it detects:**
- Duplicate explicit discriminants
- Mix of explicit and implicit discriminants (inconsistent style)
- Non-sequential numbering (informational)

**Examples:**

❌ **Vulnerable Code:**
```rust
#[contracterror]
pub enum Error {
    NotFound = 1,
    Invalid = 1,      // VIOLATION: Duplicate discriminant!
    Unauthorized = 2,
}

#[contracterror]
pub enum ErrorInconsistent {
    NotFound = 1,
    Invalid,          // VIOLATION: Mixing explicit and implicit
    Unauthorized = 3,
}
```

✅ **Clean Code:**
```rust
#[contracterror]
pub enum Error {
    NotFound = 1,
    Invalid = 2,
    Unauthorized = 3,
}

// Or all implicit:
#[contracterror]
pub enum Error {
    NotFound,
    Invalid,
    Unauthorized,
}
```

**Fix Suggestion:**  
Assign explicit unique discriminants to all error variants, or use implicit numbering consistently.

---

### 3. SANCT_EDGE_AMOUNT (S013) - Edge Case Amount Validation

**Finding Code:** `S013`  
**Category:** `code_hygiene`  
**Severity:** Warning

**Description:**  
Detects transfer/mint/burn functions missing `amount > 0` or `from != to` validation guards. These edge cases can lead to unexpected behavior or exploits.

**What it detects:**
- Transfer/mint/burn functions with `amount` parameter but no `amount > 0` check
- Transfer functions with `from` and `to` parameters but no `from != to` check

**Examples:**

❌ **Vulnerable Code:**
```rust
pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
    from.require_auth();
    // VIOLATION: Missing amount > 0 check
    // VIOLATION: Missing from != to check
    let balance = get_balance(&env, from);
    set_balance(&env, from, balance - amount);
}

pub fn mint(env: Env, to: Address, amount: i128) {
    // VIOLATION: Missing amount > 0 check
    let balance = get_balance(&env, to);
    set_balance(&env, to, balance + amount);
}
```

✅ **Clean Code:**
```rust
pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
    from.require_auth();
    
    // Proper validation
    if amount <= 0 {
        panic_with_error!(&env, Error::Invalid);
    }
    if from == to {
        panic_with_error!(&env, Error::Invalid);
    }
    
    let balance = get_balance(&env, from);
    set_balance(&env, from, balance - amount);
}

pub fn mint(env: Env, to: Address, amount: i128) {
    if amount <= 0 {
        panic_with_error!(&env, Error::Invalid);
    }
    let balance = get_balance(&env, to);
    set_balance(&env, to, balance + amount);
}
```

**Fix Suggestions:**
- For amount validation: `if amount <= 0 { panic_with_error!(...) }`
- For self-transfer: `if from == to { panic_with_error!(...) }`

---

## Implementation Architecture

### Shared Infrastructure

All three rules follow the established pattern:

1. **Rule Trait Implementation:**
   - `name()`: Returns rule identifier
   - `description()`: Human-readable description
   - `check(source: &str)`: Performs AST analysis and returns violations
   - `as_any()`: Type casting support

2. **AST Visitor Pattern:**
   - Uses `syn::visit::Visit` trait for traversal
   - Tracks context (current function, visibility, etc.)
   - Deduplicates issues with `HashSet` where needed

3. **Finding Code Registration:**
   - Codes S012-S016 added to `finding_codes.rs`
   - Includes category, description, and severity mappings

4. **Rule Registry:**
   - Rules registered in `RuleRegistry::with_default_rules()`
   - Integrated into existing detector pipeline

### Files Modified/Added

```
tooling/sanctifier-core/src/
├── finding_codes.rs                    # Modified: Added S012-S016
├── rules/
│   ├── mod.rs                          # Modified: Added new modules + registry
│   ├── hardcoded_addr.rs               # NEW
│   ├── error_code_collision.rs         # NEW
│   └── edge_amount.rs                  # NEW
└── tests/
    ├── fixtures/
    │   ├── hygiene_violations.rs       # NEW: Vulnerable code samples
    │   └── hygiene_clean.rs            # NEW: Clean code samples
    └── hygiene_rules_test.rs           # NEW: Integration tests
```

## Test Coverage

### Unit Tests (in each rule file)
- **hardcoded_addr**: 3 tests covering stellar addresses, secrets, normal strings
- **error_code_collision**: 4 tests covering duplicates, inconsistency, valid cases
- **edge_amount**: 4 tests covering amount checks, self-transfers, validation

### Integration Tests (`hygiene_rules_test.rs`)
- Comprehensive fixture-based testing with vulnerable and clean code
- Tests for all violation types across all three rules
- Severity level validation
- Combined rule execution testing

### Test Fixtures
- **hygiene_violations.rs**: Contract with intentional violations for all rules
- **hygiene_clean.rs**: Properly written contract following all best practices

## Usage

### CLI Usage

```bash
# Run all rules including hygiene pack
sanctifier check contract.rs

# Run specific hygiene rules
sanctifier check --rule hardcoded_addr contract.rs
sanctifier check --rule error_code_collision contract.rs
sanctifier check --rule edge_amount contract.rs
```

### Programmatic Usage

```rust
use sanctifier_core::rules::{
    RuleRegistry,
    hardcoded_addr::HardcodedAddrRule,
    error_code_collision::ErrorCodeCollisionRule,
    edge_amount::EdgeAmountRule,
};

let registry = RuleRegistry::with_default_rules();
let violations = registry.run_all(source_code);

// Or use individual rules
let hardcoded_rule = HardcodedAddrRule::new();
let violations = hardcoded_rule.check(source_code);
```

## Future Work (Not in this PR)

The following rules from issue #523 are planned for future PRs:

### SANCT_DEPRECATED (S014) - Deprecated SDK Functions
- Data-driven deprecation map
- Suggested replacements for deprecated soroban-sdk functions
- Requires maintaining deprecation database

### SANCT_DEAD_CODE (S015) - Dead Code Detection
- Constant-folding analysis
- Always-true/always-false condition detection
- Basic reachability analysis
- More complex than the three implemented rules

## Performance Considerations

- All rules use single-pass AST traversal
- No expensive operations like SMT solving
- Minimal memory overhead with lazy evaluation
- Rules can be disabled individually if needed

## Maintenance

### Adding New Detection Patterns

To extend existing rules:

1. **hardcoded_addr**: Update `is_address_like()` with new patterns
2. **error_code_collision**: Add new consistency checks in `check()`
3. **edge_amount**: Add new token operations in `is_token_operation()`

### Best Practices for Rule Development

- Always provide clear, actionable error messages
- Include fix suggestions where possible
- Use appropriate severity levels
- Add comprehensive test coverage
- Document detection logic and edge cases

## References

- Issue #523: [DETECTOR] Code-hygiene & best-practice lint pack
- Architecture: `ARCHITECTURE.md`
- Finding Codes: `docs/error-codes.md`
- Existing Rules: `tooling/sanctifier-core/src/rules/`
