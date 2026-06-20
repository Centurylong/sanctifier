# Getting Started with Sanctifier: Quick Examples

This guide provides quick, copy-pasteable examples to help you try Sanctifier for the first time. For a comprehensive installation and configuration guide, see [Getting Started](getting-started.md).

## 1. Quick Installation

Install the Sanctifier CLI directly from the source:

```bash
git clone https://github.com/Hypersecured/sanctifier.git
cd sanctifier
cargo install --path tooling/sanctifier-cli
```

Verify your installation:

```bash
sanctifier --version
```

## 2. Example: Analyzing a Vulnerable Contract

Create a new directory for your test project:

```bash
mkdir sanctifier-demo && cd sanctifier-demo
cargo init --lib
```

Replace the contents of `src/lib.rs` with this intentionally vulnerable Soroban smart contract.

**`src/lib.rs`**
```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol, Address};

#[contract]
pub struct VulnerableToken;

#[contractimpl]
impl VulnerableToken {
    /// VULNERABILITY: Missing require_auth()! Anyone can transfer funds.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let mut from_balance: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let mut to_balance: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        
        // VULNERABILITY: Unchecked arithmetic can overflow/underflow!
        from_balance -= amount;
        to_balance += amount;
        
        env.storage().persistent().set(&from, &from_balance);
        env.storage().persistent().set(&to, &to_balance);
        
        // VULNERABILITY: Event uses an unoptimized string topic instead of a symbol
        env.events().publish((Symbol::new(&env, "transfer"), "token_symbol"), amount);
    }
}
```

Now, run Sanctifier to analyze the contract:

```bash
sanctifier analyze .
```

### Sample Output

You should see Sanctifier identify the vulnerabilities we intentionally left in the contract:

```text
✨ Sanctifier: Valid Soroban project found at "."
🔍 Analyzing contract at "."...
✅ Static analysis complete.

🛑 Found potential Authentication Gaps!
   -> Function `transfer` is modifying state without require_auth()

🔢 Found unchecked Arithmetic Operations!
   -> Function `transfer`: Unchecked `-` (src/lib.rs:transfer)
      💡 Use checked_sub() or saturating_sub() to prevent overflow.
   -> Function `transfer`: Unchecked `+` (src/lib.rs:transfer)
      💡 Use checked_add() or saturating_add() to prevent overflow.

🔔 Found Event Consistency Issues!
   💡 Function `transfer`: Topic "token_symbol" is a long string; consider `symbol_short!`
```

## 3. Example: Fixing the Contract

Now let's secure the contract by applying Sanctifier's suggested fixes. Replace the `src/lib.rs` content with the secured version:

**`src/lib.rs`**
```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Symbol, Address, symbol_short};

#[contract]
pub struct SecuredToken;

#[contractimpl]
impl SecuredToken {
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        // FIX: Verify authorization
        from.require_auth();

        let mut from_balance: i128 = env.storage().persistent().get(&from).unwrap_or(0);
        let mut to_balance: i128 = env.storage().persistent().get(&to).unwrap_or(0);
        
        // FIX: Use checked arithmetic to prevent panics or exploits
        from_balance = from_balance.checked_sub(amount).expect("Insufficient balance or underflow");
        to_balance = to_balance.checked_add(amount).expect("Balance overflow");
        
        env.storage().persistent().set(&from, &from_balance);
        env.storage().persistent().set(&to, &to_balance);
        
        // FIX: Use symbol_short! for optimized token indexing
        env.events().publish((symbol_short!("transfer"), symbol_short!("TOKEN")), amount);
    }
}
```

Run Sanctifier again to verify the fixes:

```bash
sanctifier analyze .
```

You should see a clean run:

```text
✨ Sanctifier: Valid Soroban project found at "."
🔍 Analyzing contract at "."...
✅ Static analysis complete.

🌟 Perfect! No vulnerabilities or warnings found in your contract.
```

## Next Steps
To continue configuring Sanctifier for your specific project needs, or to learn about Formal Verification, check out the [Detailed Setup & Configuration Guide](getting-started.md). 
