# Reentrancy Pattern Detection

## Overview

Sanctifier includes advanced static analysis to detect risky call patterns that might lead to reentrancy vulnerabilities in Soroban smart contracts. While Soroban prevents classical cross-contract reentrancy at the runtime level, state-based reentrancy and complex interaction patterns can still pose security risks.

## Detected Patterns

### 1. External Call in Loop (High Severity)

**Pattern**: External contract calls inside loop constructs (`for`, `while`, `loop`)

**Risk**:

- Reentrancy attacks can be amplified across iterations
- Excessive gas consumption
- Unpredictable state changes during iteration

**Example**:

```rust
pub fn batch_transfer(env: Env, recipients: Vec<Address>) {
    for recipient in recipients.iter() {
        // ❌ RISKY: External call in loop
        token_client.transfer(&env.current_contract_address(), &recipient, &100);
    }
}
```

**Recommendation**:

- Batch operations outside the loop
- Use reentrancy guard if batching isn't possible
- Consider gas limits and transaction size

### 2. Multiple External Calls (High Severity)

**Pattern**: Functions making 2 or more external calls without reentrancy protection

**Risk**:

- Increased attack surface
- Complex reentrancy scenarios between calls
- State can be manipulated between calls

**Example**:

```rust
pub fn complex_operation(env: Env) {
    // ❌ RISKY: Multiple external calls without guard
    token_client.approve(&spender, &amount);
    vault_client.deposit(&amount);
    oracle_client.update_price(&asset);
}
```

**Recommendation**:

- Use `ReentrancyGuardian.enter(nonce)` / `.exit()` to protect the entire function
- Consider breaking into separate guarded functions
- Follow Checks-Effects-Interactions pattern

### 3. State After Call (High Severity)

**Pattern**: State mutations occurring after external calls

**Risk**:

- Violates Checks-Effects-Interactions (CEI) pattern
- State changes can be observed and exploited by called contract
- Callback attacks can manipulate post-call state

**Example**:

```rust
pub fn risky_flow(env: Env) {
    external_client.transfer(&to, &amount);
    // ❌ RISKY: State mutation after external call
    env.storage().instance().set(&"last_transfer", &amount);
}
```

**Recommendation**:

- Move all state changes before external calls
- Use reentrancy guard if state must be updated after call
- Follow CEI pattern: Checks → Effects → Interactions

### 4. Critical CEI Violation (High Severity)

**Pattern**: State mutations both before AND after external calls

**Risk**:

- Most dangerous pattern
- State can be manipulated mid-execution
- Multiple attack vectors
- Complex to reason about security

**Example**:

```rust
pub fn very_dangerous(env: Env) {
    // ❌ CRITICAL: State mutation before call
    env.storage().instance().set(&"status", &1u32);

    external_client.do_something();

    // ❌ CRITICAL: State mutation after call
    env.storage().instance().set(&"completed", &true);
}
```

**Recommendation**:

- Restructure to follow CEI pattern strictly
- Use reentrancy guard for the entire function
- Consider splitting into separate functions with guards

### 5. State Before Call (Medium Severity)

**Pattern**: Classic reentrancy - state mutation followed by external call

**Risk**:

- Traditional reentrancy vulnerability
- State changes visible to called contract
- Can be exploited if guard is missing

**Example**:

```rust
pub fn withdraw(env: Env, amount: i128) {
    let balance: i128 = env.storage().instance().get(&"balance").unwrap();
    // ❌ RISKY: State mutation before external call
    env.storage().instance().set(&"balance", &(balance - amount));
    token_client.transfer(&env.current_contract_address(), &caller, &amount);
}
```

**Recommendation**:

- Use `ReentrancyGuardian.enter(nonce)` / `.exit()`
- Or follow CEI pattern: perform call before state changes if possible
- Add proper checks before state mutations

## Safe Patterns

### Using Reentrancy Guardian

```rust
pub fn safe_withdraw(env: Env, nonce: u64, amount: i128) {
    // ✅ SAFE: Protected by reentrancy guard
    guardian.enter(nonce);

    let balance: i128 = env.storage().instance().get(&"balance").unwrap();
    env.storage().instance().set(&"balance", &(balance - amount));
    token_client.transfer(&env.current_contract_address(), &caller, &amount);

    guardian.exit();
}
```

### Following CEI Pattern

```rust
pub fn proper_withdraw(env: Env, amount: i128) {
    // ✅ Checks
    let balance: i128 = env.storage().instance().get(&"balance").unwrap();
    assert!(balance >= amount);

    // ✅ Effects (state changes)
    env.storage().instance().set(&"balance", &(balance - amount));

    // ✅ Interactions (external calls last)
    token_client.transfer(&env.current_contract_address(), &caller, &amount);
}
```

### Read-Only Operations

```rust
pub fn query_data(env: Env) -> u64 {
    // ✅ SAFE: No state mutations
    let value: u64 = env.storage().instance().get(&"data").unwrap_or(0);
    let price = external_client.get_price(&asset);
    value + price
}
```

## Detection Capabilities

The analyzer tracks:

- **Statement Order**: Detects the sequence of state mutations and external calls
- **Loop Contexts**: Identifies external calls within any loop construct
- **Call Counting**: Tracks multiple external calls in a single function
- **Guard Recognition**: Recognizes various guard naming patterns:
  - `guardian.enter()` / `guardian.exit()`
  - `guard.enter()` / `guard.exit()`
  - `reentrancy_lock.enter()` / `reentrancy_lock.exit()`
- **External Call Methods**:
  - Client pattern: `*_client.method()`
  - Direct invocation: `invoke_contract()`
  - Auth-checked: `invoke_contract_check_auth()`
- **Storage Operations**: All storage types (instance, persistent, temporary)

## Running Analysis

### CLI Usage

```bash
# Analyze a single contract
sanctifier analyze ./contracts/my-contract

# Analyze with JSON output
sanctifier analyze ./contracts/my-contract --format json

# Analyze entire workspace
sanctifier analyze ./contracts
```

### Programmatic Usage

```rust
use sanctifier_core::{Analyzer, SanctifyConfig};

let analyzer = Analyzer::new(SanctifyConfig::default());
let source = std::fs::read_to_string("contract.rs")?;
let issues = analyzer.scan_reentrancy(&source);

for issue in issues {
    println!("Function: {}", issue.function_name);
    println!("Severity: {}", issue.severity);
    println!("Pattern: {:?}", issue.pattern);
    println!("Recommendation: {}", issue.recommendation);
}
```

## Integration with CI/CD

### GitHub Actions Example

```yaml
name: Security Scan

on: [push, pull_request]

jobs:
  sanctifier:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Sanctifier
        run: cargo install sanctifier-cli
      - name: Run Reentrancy Analysis
        run: sanctifier analyze ./contracts --format json > results.json
      - name: Check for High Severity Issues
        run: |
          HIGH_COUNT=$(jq '[.reentrancy_issues[] | select(.severity == "high")] | length' results.json)
          if [ "$HIGH_COUNT" -gt 0 ]; then
            echo "Found $HIGH_COUNT high severity reentrancy issues!"
            exit 1
          fi
```

## Best Practices

1. **Always Use Guards for Complex Flows**
   - Multi-step operations with external calls
   - Functions that both read and write state around calls

2. **Follow CEI Pattern When Possible**
   - Perform all checks first
   - Update all state second
   - Make external calls last

3. **Avoid External Calls in Loops**
   - Batch operations when possible
   - Use events for notifications instead of callbacks

4. **Test Reentrancy Scenarios**
   - Write tests that simulate reentrancy attacks
   - Use the ReentrancyGuardian contract in tests

5. **Regular Security Audits**
   - Run Sanctifier on every commit
   - Review all high-severity findings
   - Document why patterns are safe if analyzer flags them

## Limitations

- **Static Analysis Only**: Cannot detect runtime-specific vulnerabilities
- **False Positives**: Some safe patterns may be flagged (e.g., read-only external calls)
- **Naming Conventions**: Guard detection relies on naming patterns
- **Complex Control Flow**: May not catch all patterns in highly complex code

## Further Reading

- [Reentrancy Guardian Documentation](./reentrancy-guardian.md)
- [Checks-Effects-Interactions Pattern](https://docs.soliditylang.org/en/latest/security-considerations.html#use-the-checks-effects-interactions-pattern)
- [Soroban Security Best Practices](https://soroban.stellar.org/docs/learn/security)
