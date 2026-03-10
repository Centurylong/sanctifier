# Storage Type Validator

The Storage Type Validator ensures that persistent storage is only used for data that actually needs to live forever, following Soroban best practices for optimal resource usage and cost efficiency.

## Overview

Soroban provides three types of storage, each with different characteristics and costs:

- **Persistent Storage**: Data survives contract upgrades and persists indefinitely. Most expensive.
- **Instance Storage**: Data persists for the lifetime of the contract instance. Moderate cost.
- **Temporary Storage**: Data only exists for the duration of the current transaction. Least expensive.

The Storage Type Validator analyzes your contract code to identify cases where data is stored in an inappropriate storage type, potentially leading to:

- Unnecessary storage costs
- Resource waste
- Data loss risks
- Violation of Soroban best practices

## Detection Patterns

The validator uses intelligent pattern recognition to categorize data usage:

### Temporary Data (should use Temporary storage)

- Keys containing: `temp`, `cache`, `tmp`
- Short-lived calculations
- Intermediate results

### Session Data (should use Instance storage)

- Keys containing: `session`, `nonce`, `lock`
- User session information
- Temporary state that needs to persist across calls within a session

### Configuration Data (should use Persistent storage)

- Keys containing: `config`, `admin`, `owner`
- Contract configuration
- Administrative settings

### Critical Financial Data (should use Persistent storage)

- Keys containing: `balance`, `allowance`, `supply`
- User balances
- Token supplies
- Financial state

### Contract State (should use Persistent storage)

- Keys containing: `state`, `status`
- Core contract state
- Status information

## Issue Types and Severity

### High Severity Issues

1. **Critical data in temporary storage**
   - Risk: Data loss after transaction completion
   - Example: User balances, token supplies stored in temporary storage

2. **Temporary data in persistent storage**
   - Risk: Unnecessary storage costs and resource waste
   - Example: Cache values, temporary calculations stored in persistent storage

### Medium Severity Issues

1. **Session data in persistent storage**
   - Risk: Suboptimal storage costs
   - Example: Session IDs, nonces stored in persistent storage

### Low Severity Issues

1. **General storage type mismatches**
   - Risk: Minor inefficiencies
   - Example: Unclear usage patterns with suboptimal storage choices

## Usage

### CLI Analysis

```bash
# Analyze a single contract file
sanctifier-cli analyze contract.rs

# Get JSON output with detailed storage type issues
sanctifier-cli analyze contract.rs --format json
```

### Programmatic Usage

```rust
use sanctifier_core::{Analyzer, SanctifyConfig};

let analyzer = Analyzer::new(SanctifyConfig::default());
let issues = analyzer.scan_storage_type_validation(source_code);

for issue in issues {
    println!("Function: {}", issue.function_name);
    println!("Key: {}", issue.key);
    println!("Current: {} -> Recommended: {}",
             issue.current_storage_type,
             issue.recommended_storage_type);
    println!("Reason: {}", issue.reason);
    println!("Severity: {}", issue.severity);
}
```

## Example Issues

### Bad: Temporary data in persistent storage

```rust
#[contractimpl]
impl MyContract {
    pub fn calculate(env: Env, value: i128) {
        // ❌ BAD: Temporary calculation stored persistently
        env.storage().persistent().set(&"temp_result", &(value * 2));
        env.storage().persistent().set(&"cache_data", &42);
    }
}
```

**Issues detected:**

- `temp_result`: Persistent → Temporary (HIGH severity)
- `cache_data`: Persistent → Temporary (HIGH severity)

### Bad: Critical data in temporary storage

```rust
#[contractimpl]
impl TokenContract {
    pub fn transfer(env: Env, amount: i128) {
        // ❌ BAD: Critical financial data in temporary storage
        env.storage().temporary().set(&"user_balance", &amount);
        env.storage().temporary().set(&"total_supply", &1000000);
    }
}
```

**Issues detected:**

- `user_balance`: Temporary → Persistent (HIGH severity)
- `total_supply`: Temporary → Persistent (HIGH severity)

### Good: Appropriate storage usage

```rust
#[contractimpl]
impl MyContract {
    pub fn proper_storage(env: Env, user: Address, amount: i128) {
        // ✅ GOOD: Critical data in persistent storage
        env.storage().persistent().set(&"user_balance", &amount);
        env.storage().persistent().set(&"admin_address", &user);

        // ✅ GOOD: Session data in instance storage
        env.storage().instance().set(&"session_id", &"abc123");
        env.storage().instance().set(&"nonce", &123);

        // ✅ GOOD: Temporary data in temporary storage
        env.storage().temporary().set(&"temp_calculation", &(amount * 2));
        env.storage().temporary().set(&"cache_result", &42);
    }
}
```

**Result:** No issues detected - all storage types are appropriate.

## Best Practices

1. **Use Persistent storage for:**
   - User balances and financial data
   - Contract configuration and admin settings
   - Core contract state that must survive upgrades
   - Any data that needs to persist indefinitely

2. **Use Instance storage for:**
   - Session-based data
   - Temporary state that needs to persist across multiple calls
   - User preferences that don't need to survive contract upgrades
   - Nonces and locks

3. **Use Temporary storage for:**
   - Cache data
   - Intermediate calculations
   - Data that only needs to exist during the current transaction
   - Temporary results and working variables

4. **Consider storage costs:**
   - Persistent storage is the most expensive
   - Instance storage has moderate costs
   - Temporary storage is the least expensive

## Integration

The Storage Type Validator is automatically included in Sanctifier's comprehensive analysis and integrates with:

- CLI analysis reports
- JSON output for CI/CD integration
- Sanctity score calculation
- Custom rule engines

## Configuration

The validator uses intelligent pattern recognition and doesn't require additional configuration. However, you can extend the analysis by:

1. Adding custom rules for domain-specific patterns
2. Integrating with your contract's specific naming conventions
3. Customizing severity levels based on your project requirements

## Limitations

- Pattern recognition is based on key naming conventions
- Complex dynamic key generation may not be fully analyzed
- Requires clear, descriptive key names for optimal detection
- May produce false positives for unconventional naming patterns

## Contributing

To improve the Storage Type Validator:

1. Add new pattern recognition rules
2. Enhance context analysis for better recommendations
3. Improve severity classification
4. Add support for custom naming conventions
