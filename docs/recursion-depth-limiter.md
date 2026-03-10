# Recursion Depth Limiter Check

## Overview

The Recursion Depth Limiter is a static analysis check that detects potentially recursive calls in Soroban smart contracts that could exceed stack limits. Soroban has limited stack depth, and unbounded or deep recursion can cause contract failures.

## Why This Matters

Soroban smart contracts run in a constrained environment with limited stack space. Recursive function calls can quickly exhaust the available stack, leading to:

- Contract execution failures
- Unpredictable behavior
- Potential security vulnerabilities
- Poor user experience

## Types of Recursion Detected

### 1. Direct Recursion

A function that calls itself directly.

**Example:**

```rust
pub fn factorial(n: u32) -> u32 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)  // Direct recursion
    }
}
```

### 2. Indirect Recursion

A function that calls itself through one or more intermediate functions.

**Example:**

```rust
pub fn process_a(n: u32) -> u32 {
    if n > 0 {
        process_b(n)  // Calls B
    } else {
        0
    }
}

fn process_b(n: u32) -> u32 {
    if n > 1 {
        process_a(n - 1)  // Calls A - creates cycle
    } else {
        1
    }
}
```

## How It Works

The recursion analyzer:

1. **Builds a Call Graph**: Parses the contract source code and constructs a graph of function calls
2. **Detects Cycles**: Uses depth-first search to identify cycles in the call graph
3. **Classifies Recursion**: Determines whether recursion is direct or indirect
4. **Reports Issues**: Provides detailed information about each recursive pattern found

## Usage

### CLI

```bash
# Analyze a single file
sanctifier analyze path/to/contract.rs

# Analyze a directory
sanctifier analyze path/to/contracts/

# JSON output
sanctifier analyze path/to/contract.rs --format json
```

### Output Format

**Text Output:**

```
🔄 Found Recursive Call Patterns!
   -> Direct: Function 'factorial' calls itself directly, which may exceed Soroban stack limits
      📍 Call chain: factorial -> factorial
   -> Indirect: Function 'process_a' is part of a recursive call chain: process_a -> process_b -> process_a
      📍 Call chain: process_a -> process_b -> process_a -> process_a
   💡 Tip: Soroban has limited stack depth. Consider iterative approaches or bounded recursion.
```

**JSON Output:**

```json
{
  "recursion_issues": [
    {
      "function_name": "factorial",
      "recursion_type": "Direct",
      "call_chain": ["factorial", "factorial"],
      "estimated_depth": null,
      "message": "Function 'factorial' calls itself directly, which may exceed Soroban stack limits",
      "location": "factorial"
    }
  ]
}
```

## Recommendations

When recursion is detected, consider these alternatives:

### 1. Use Iteration Instead

**Before (Recursive):**

```rust
pub fn factorial(n: u32) -> u32 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}
```

**After (Iterative):**

```rust
pub fn factorial(n: u32) -> u32 {
    let mut result = 1;
    for i in 2..=n {
        result *= i;
    }
    result
}
```

### 2. Add Depth Limits

If recursion is necessary, add explicit depth limits:

```rust
pub fn process(env: Env, n: u32, depth: u32) -> Result<u32, Error> {
    const MAX_DEPTH: u32 = 10;

    if depth >= MAX_DEPTH {
        return Err(Error::MaxDepthExceeded);
    }

    if n <= 1 {
        Ok(1)
    } else {
        process(env, n - 1, depth + 1)
    }
}
```

### 3. Use Tail Recursion

Some recursive patterns can be optimized by the compiler if written as tail recursion:

```rust
pub fn factorial_tail(n: u32, acc: u32) -> u32 {
    if n <= 1 {
        acc
    } else {
        factorial_tail(n - 1, n * acc)
    }
}
```

### 4. Restructure the Algorithm

Consider if the algorithm can be restructured to avoid recursion entirely:

- Use queues or stacks for tree/graph traversal
- Use dynamic programming for recursive problems
- Break down the problem into smaller, non-recursive steps

## Integration with CI/CD

Add Sanctifier to your CI pipeline to catch recursion issues early:

```yaml
# .github/workflows/security.yml
name: Security Analysis
on: [push, pull_request]

jobs:
  sanctifier:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Install Sanctifier
        run: cargo install --path tooling/sanctifier-cli
      - name: Run Analysis
        run: sanctifier analyze contracts/ --format json > analysis.json
      - name: Check for Recursion
        run: |
          if jq -e '.recursion_issues | length > 0' analysis.json; then
            echo "Recursion detected!"
            exit 1
          fi
```

## Limitations

- **False Positives**: The analyzer may flag bounded recursion that is actually safe
- **Dynamic Dispatch**: Recursion through trait objects or function pointers may not be detected
- **Cross-Contract Calls**: Recursion through external contract calls is not detected
- **Conditional Recursion**: The analyzer doesn't evaluate conditions, so it may flag recursion that never actually occurs at runtime

## Configuration

Currently, the recursion check is always enabled. Future versions may support:

- Configurable depth limits
- Whitelisting specific functions
- Severity levels based on call chain depth

## Technical Details

### Implementation

The recursion analyzer is implemented in `tooling/sanctifier-core/src/recursion.rs` and uses:

- **syn**: For parsing Rust source code into an AST
- **Call Graph Construction**: Visitor pattern to build function call relationships
- **Cycle Detection**: Depth-first search with path tracking

### Performance

The analyzer is designed to be fast and efficient:

- Linear time complexity for call graph construction: O(n) where n is the number of functions
- Cycle detection: O(V + E) where V is vertices (functions) and E is edges (calls)
- Minimal memory overhead

## Examples

See `contracts/test-recursion/` for example contracts with various recursion patterns.

## Contributing

To improve the recursion analyzer:

1. Add test cases in `tooling/sanctifier-core/src/tests/recursion_tests.rs`
2. Enhance detection algorithms in `tooling/sanctifier-core/src/recursion.rs`
3. Update documentation with new patterns or recommendations

## References

- [Soroban Documentation](https://soroban.stellar.org/)
- [Stellar Stack Limits](https://soroban.stellar.org/docs/learn/encyclopedia/contract-development/environment-concepts)
- [Recursion in Smart Contracts](https://docs.soliditylang.org/en/latest/security-considerations.html#recursion)
