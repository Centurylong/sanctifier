# Soroban Secure-Coding Best Practices

A curated handbook of Soroban security patterns. Each entry pairs a **vulnerable
snippet** with a **fixed snippet** and links to the Sanctifier finding code that
catches the issue — turning Sanctifier's detectors into an educational resource.

> **Cross-references:** [Finding Code Catalog](error-codes.md) ·
> [Detector Catalog](detectors/README.md) ·
> [Glossary](glossary.md) ·
> [Case Studies](case-studies/admin-takeover.md)

---

## Quick-Reference Table

| # | Practice | Theme | Finding Code | Severity |
|---|----------|-------|-------------|----------|
| 1 | [Always call `require_auth` on state-mutating entrypoints](#1-always-call-require_auth-on-state-mutating-entrypoints) | Authentication | [`S001`](error-codes.md) | Critical |
| 2 | [Never expose helper mutators through `#[contractimpl]` without auth](#2-never-expose-helper-mutators-through-contractimpl-without-auth) | Authentication | [`SANCT_VISIBILITY`](error-codes.md) | High |
| 3 | [Load admin from storage; never hardcode privileged addresses](#3-load-admin-from-storage-never-hardcode-privileged-addresses) | Authentication | [`S012`](error-codes.md) | High |
| 4 | [Guard `init` — reject re-initialisation and avoid hardcoded admins](#4-guard-init--reject-re-initialisation-and-avoid-hardcoded-admins) | Authentication | [`SANCT_INIT_HARDCODED_ADMIN`](error-codes.md) | Warning |
| 5 | [Use `checked_add`/`checked_sub`/`checked_mul` for all token arithmetic](#5-use-checked_addchecked_subchecked_mul-for-all-token-arithmetic) | Arithmetic | [`S003`](error-codes.md) | High |
| 6 | [Guard against unsigned underflow on balance decrements](#6-guard-against-unsigned-underflow-on-balance-decrements) | Arithmetic | [`S019`](error-codes.md) | High |
| 7 | [Prevent division-by-zero on user-controlled denominators](#7-prevent-division-by-zero-on-user-controlled-denominators) | Arithmetic | [`S018`](error-codes.md) | Medium |
| 8 | [Avoid integer-division fee rounding to zero for micro-amounts](#8-avoid-integer-division-fee-rounding-to-zero-for-micro-amounts) | Arithmetic | [`S017`](error-codes.md) | High |
| 9 | [Replace bit-shift by unchecked widths with guarded shifts](#9-replace-bit-shift-by-unchecked-widths-with-guarded-shifts) | Arithmetic | [`SANCT_SHIFT_OVERFLOW`](error-codes.md) | Warning |
| 10 | [Extend TTL on every persistent/instance storage access](#10-extend-ttl-on-every-persistentinstance-storage-access) | Storage & TTL | [`S006`](error-codes.md) | Medium |
| 11 | [Keep ledger entries within the size limit](#11-keep-ledger-entries-within-the-size-limit) | Storage & TTL | [`S004`](error-codes.md) | Medium |
| 12 | [Cap persistent collections — never let storage grow unbounded](#12-cap-persistent-collections--never-let-storage-grow-unbounded) | Storage & TTL | [`SANCT_UNBOUNDED_STORAGE`](error-codes.md) | High |
| 13 | [Cap Vec/Map arguments before iterating](#13-cap-vecmap-arguments-before-iterating) | Denial of Service | [`SANCT_ARG_DOS`](error-codes.md) | High |
| 14 | [Replace `panic!`/`unwrap`/`expect` with typed errors](#14-replace-panicunwrapexpect-with-typed-errors) | Panic Handling | [`S002`](error-codes.md) | High |
| 15 | [Propagate `Result` — never silently drop it](#15-propagate-result--never-silently-drop-it) | Panic Handling | [`S009`](error-codes.md) | Medium |
| 16 | [View/getter entrypoints must not panic or write state](#16-viewgetter-entrypoints-must-not-panic-or-write-state) | Panic Handling | [`SANCT_VIEW_PANIC`](error-codes.md) / [`SANCT_STATE_WRITE_IN_VIEW`](error-codes.md) | Medium |
| 17 | [Use delta or compare-and-set semantics for `approve`](#17-use-delta-or-compare-and-set-semantics-for-approve) | Authorization | [`SANCT_ALLOWANCE_RACE`](error-codes.md) | Medium |
| 18 | [Validate edge amounts: reject zero and self-transfers](#18-validate-edge-amounts-reject-zero-and-self-transfers) | Input Validation | [`S013`](error-codes.md) | Medium |
| 19 | [Compare balances with `>=`/`<=`, not `==`/`!=`](#19-compare-balances-with--not-) | Logic | [`SANCT_BALANCE_EQ`](error-codes.md) | Info |
| 20 | [Use `timestamp()` for real-time windows, not `sequence()`](#20-use-timestamp-for-real-time-windows-not-sequence) | Time Logic | [`S021`](error-codes.md) | Medium |

---

## Theme 1 — Authentication & Access Control

### 1. Always call `require_auth` on state-mutating entrypoints

**Rationale.** Soroban invocations are public by default. Any on-chain actor can
call any exported function unless the function cryptographically proves the
relevant account authorised the call. A missing `require_auth` lets an attacker
drain balances, overwrite admin records, or trigger privileged operations with no
permission at all. This is the single most common critical bug class in Soroban
contracts.

**Vulnerable**
```rust
// contracts/token-with-bugs/src/lib.rs — reproduced for illustration
#[contractimpl]
impl Token {
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        // ❌ No require_auth — anyone can drain any account.
        let bal: i128 = e.storage().persistent().get(&from).unwrap_or(0);
        e.storage().persistent().set(&from, &(bal - amount));
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth(); // ✅ caller must prove they control `from`
        let bal: i128 = e.storage().persistent().get(&from).unwrap_or(0);
        e.storage().persistent().set(&from, &(bal - amount));
    }
}
```

**Finding code:** [`S001`](error-codes.md) — flagged by the [`auth_gap`](detectors/auth_gap.md) detector.

---

### 2. Never expose helper mutators through `#[contractimpl]` without auth

**Rationale.** A function with a "helper" or "internal" naming convention (leading
underscore, `_set_*`, `_update_*`) that is placed inside `#[contractimpl]` becomes
a callable on-chain entrypoint. If it mutates state without authentication, any
caller can invoke it directly and bypass the logic of the "real" entrypoints that
were meant to guard it.

**Vulnerable**
```rust
#[contractimpl]
impl Token {
    // ❌ Publicly callable — helper name does not make it private on-chain.
    pub fn _set_balance(env: Env, owner: Address, amount: i128) {
        env.storage().persistent().set(&owner, &amount);
    }
}
```

**Fixed**
```rust
// ✅ Move to a plain Rust function outside #[contractimpl] so it is never exported.
fn set_balance(env: &Env, owner: &Address, amount: i128) {
    env.storage().persistent().set(owner, &amount);
}

#[contractimpl]
impl Token {
    pub fn mint(env: Env, to: Address, amount: i128) {
        // authenticate here before calling the private helper
        to.require_auth();
        set_balance(&env, &to, amount);
    }
}
```

**Finding code:** [`SANCT_VISIBILITY`](error-codes.md) — flagged by the [`sanct_visibility`](detectors/sanct_visibility.md) detector.

---

### 3. Load admin from storage; never hardcode privileged addresses

**Rationale.** A hardcoded address embedded as a literal is permanent — it cannot
be rotated if the key is compromised, and it is trivially visible on-chain. Admin
addresses must be written to storage during initialisation and loaded at runtime
so ownership can be transferred through a governed process.

**Vulnerable**
```rust
#[contractimpl]
impl Governance {
    pub fn execute(env: Env, caller: Address) {
        // ❌ Hardcoded — key rotation impossible; address visible in source.
        let admin = Address::from_str(&env,
            "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN");
        if caller != admin {
            panic!("not admin");
        }
        // ... privileged action
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Governance {
    pub fn initialize(env: Env, admin: Address) {
        env.storage().instance().set(&symbol_short!("admin"), &admin);
    }

    pub fn execute(env: Env) {
        // ✅ Load admin from storage; authenticate at runtime.
        let admin: Address = env.storage().instance()
            .get(&symbol_short!("admin"))
            .expect("not initialised");
        admin.require_auth();
        // ... privileged action
    }
}
```

**Finding code:** [`S012`](error-codes.md) — flagged by the [`hardcoded_addr`](detectors/hardcoded_addr.md) detector.

---

### 4. Guard `init` — reject re-initialisation and avoid hardcoded admins

**Rationale.** An `initialize` function that can be called more than once lets an
attacker overwrite the stored admin, taking over the contract after deployment.
Equally, an `init` that sets a hardcoded privileged account defeats the purpose
of on-chain governance. Always write an initialisation-guard and accept the admin
as a parameter from the deployer.

**Vulnerable**
```rust
#[contractimpl]
impl Vault {
    pub fn initialize(env: Env) {
        // ❌ No re-init guard; hardcoded admin literal.
        let admin = Address::from_str(&env, "GAAZI4TCR3TY5...");
        env.storage().instance().set(&DataKey::Admin, &admin);
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Vault {
    pub fn initialize(env: Env, admin: Address) {
        // ✅ Reject if already initialised.
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialised");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
    }
}
```

**Finding code:** [`SANCT_INIT_HARDCODED_ADMIN`](error-codes.md) — flagged by the [`init_hardcoded_admin`](detectors/init_hardcoded_admin.md) detector.

---

## Theme 2 — Arithmetic Safety

### 5. Use `checked_add`/`checked_sub`/`checked_mul` for all token arithmetic

**Rationale.** Soroban contracts compile to WASM and run in release mode where
Rust integer arithmetic **wraps silently** on overflow. An attacker who can
cause `balance + mint_amount` to wrap past `i128::MAX` can manufacture tokens
from thin air or bypass a supply cap. Use the `checked_*` family and convert
`None` into a typed contract error.

**Vulnerable**
```rust
// contracts/token-with-bugs/src/lib.rs — reproduced for illustration
#[contractimpl]
impl Token {
    pub fn mint(e: Env, to: Address, amount: i128) {
        let bal = Self::balance(e.clone(), to.clone());
        // ❌ Wraps past i128::MAX in release — supply cap bypass.
        e.storage().persistent().set(&to, &(bal + amount));
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    pub fn mint(e: Env, to: Address, amount: i128) -> Result<(), Error> {
        let bal = Self::balance(e.clone(), to.clone());
        // ✅ Returns Err on overflow instead of wrapping.
        let new_bal = bal.checked_add(amount).ok_or(Error::Overflow)?;
        e.storage().persistent().set(&to, &new_bal);
        Ok(())
    }
}
```

**Finding code:** [`S003`](error-codes.md) — flagged by the [`arithmetic_overflow`](detectors/arithmetic_overflow.md) detector.

---

### 6. Guard against unsigned underflow on balance decrements

**Rationale.** Subtracting from a `u64`/`u32` balance without checking that
`from_balance >= amount` wraps the result to a huge positive number, effectively
granting an unlimited balance. Even with `i128` this is a logic error: a
negative balance should never silently be stored. Validate or use `checked_sub`.

**Vulnerable**
```rust
#[contractimpl]
impl Token {
    pub fn burn(e: Env, from: Address, amount: u64) {
        from.require_auth();
        let bal: u64 = e.storage().persistent().get(&from).unwrap_or(0);
        // ❌ Wraps to u64::MAX when bal < amount.
        e.storage().persistent().set(&from, &(bal - amount));
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    pub fn burn(e: Env, from: Address, amount: u64) -> Result<(), Error> {
        from.require_auth();
        let bal: u64 = e.storage().persistent().get(&from).unwrap_or(0);
        // ✅ Returns Err::InsufficientBalance instead of underflowing.
        let new_bal = bal.checked_sub(amount).ok_or(Error::InsufficientBalance)?;
        e.storage().persistent().set(&from, &new_bal);
        Ok(())
    }
}
```

**Finding code:** [`S019`](error-codes.md) — flagged by the [`unsigned_underflow`](detectors/unsigned_underflow.md) detector.

---

### 7. Prevent division-by-zero on user-controlled denominators

**Rationale.** Any `/` or `%` whose denominator comes from user input, contract
storage, or a computed value can be zero. In Soroban's WASM host a division by
zero panics the transaction, so an attacker can use a zero denominator to
selectively DoS any operation that divides by it (e.g. share price, fee rate).
Always validate the denominator is non-zero before dividing.

**Vulnerable**
```rust
#[contractimpl]
impl Pool {
    pub fn price(e: Env, reserve_a: i128, reserve_b: i128) -> i128 {
        // ❌ Panics when reserve_b is 0 (pool drained or uninitialised).
        reserve_a / reserve_b
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Pool {
    pub fn price(e: Env, reserve_a: i128, reserve_b: i128) -> Result<i128, Error> {
        // ✅ Explicit guard before dividing.
        if reserve_b == 0 {
            return Err(Error::DivisionByZero);
        }
        Ok(reserve_a / reserve_b)
    }
}
```

**Finding code:** [`S018`](error-codes.md) — flagged by the [`division_by_zero`](detectors/division_by_zero.md) detector.

---

### 8. Avoid integer-division fee rounding to zero for micro-amounts

**Rationale.** Integer division truncates toward zero. A fee formula such as
`amount * rate / 10_000` produces zero whenever `amount * rate < 10_000`. An
attacker can repeatedly send micro-amounts that pay zero fees, draining the
protocol over time. Scale the numerator first or use basis-point math that
rounds up.

**Vulnerable**
```rust
pub fn calculate_fee(amount: i128, bps: i128) -> i128 {
    // ❌ Returns 0 for any amount < 10_000 / bps.
    amount * bps / 10_000
}
```

**Fixed**
```rust
pub fn calculate_fee(amount: i128, bps: i128) -> i128 {
    // ✅ Ceiling division: always charges at least 1 stroop when amount > 0.
    (amount * bps + 9_999) / 10_000
}
```

**Finding code:** [`S017`](error-codes.md) — flagged by the [`fee_rounding`](detectors/fee_rounding.md) detector.

---

### 9. Replace bit-shift by unchecked widths with guarded shifts

**Rationale.** Shifting an integer by an amount `>= bit_width` is undefined
behaviour in C and wraps or panics in Rust depending on mode. In a contract,
the shift amount often comes from user input or storage. A shift of 64+ bits on
a `u64` produces garbage values or aborts the call. Validate the shift amount
before applying it.

**Vulnerable**
```rust
pub fn scale(value: u64, shift: u32) -> u64 {
    // ❌ Panics in debug / wraps in release when shift >= 64.
    value << shift
}
```

**Fixed**
```rust
pub fn scale(value: u64, shift: u32) -> Result<u64, Error> {
    // ✅ Reject out-of-range shifts before they execute.
    if shift >= 64 {
        return Err(Error::ShiftOutOfRange);
    }
    Ok(value << shift)
}
```

**Finding code:** [`SANCT_SHIFT_OVERFLOW`](error-codes.md) — flagged by the [`shift_overflow`](detectors/shift_overflow.md) detector.

---

## Theme 3 — Storage & TTL

### 10. Extend TTL on every persistent/instance storage access

**Rationale.** In Soroban, persistent and instance storage entries have a
time-to-live measured in ledgers. Once a TTL expires, the entry is *archived* —
the contract can no longer read it until it is explicitly restored. A contract
that never calls `extend_ttl` will have its balances, config, and state silently
archived, freezing user funds and bricking the contract without any explicit
attack.

**Vulnerable**
```rust
#[contractimpl]
impl Vault {
    pub fn deposit(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let key = DataKey::Balance(user);
        let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        // ❌ No extend_ttl — balance will be archived after the TTL lapses.
        env.storage().persistent().set(&key, &(bal + amount));
    }
}
```

**Fixed**
```rust
const DAY_LEDGERS: u32 = 17_280; // ~1 day
const BUMP: u32 = 30 * DAY_LEDGERS;
const THRESHOLD: u32 = BUMP - DAY_LEDGERS;

#[contractimpl]
impl Vault {
    pub fn deposit(env: Env, user: Address, amount: i128) {
        user.require_auth();
        let key = DataKey::Balance(user);
        let bal: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(bal + amount));
        // ✅ Bump TTL after every write so the entry stays live.
        env.storage().persistent().extend_ttl(&key, THRESHOLD, BUMP);
    }
}
```

**Finding code:** [`S006`](error-codes.md) — flagged by the [`missing_ttl`](detectors/missing_ttl.md) detector.

---

### 11. Keep ledger entries within the size limit

**Rationale.** The Soroban host enforces a hard limit on ledger entry size
(~64 KB). A `contracttype` struct or a collection stored in a single entry that
grows near this limit will cause writes to fail, bricking the affected path.
Keep data types small; split large structures across multiple keyed entries.

**Vulnerable**
```rust
#[contracttype]
pub struct Config {
    pub admin: Address,
    pub whitelist: Vec<Address>, // ❌ Can grow until the entry exceeds 64 KB.
    pub history: Vec<i128>,      // ❌ Unbounded history in a single entry.
}
```

**Fixed**
```rust
#[contracttype]
pub struct Config {
    pub admin: Address,
    // ✅ Cap in-entry collections or key each element individually.
}

// Store whitelist entries as separate keyed records so each stays small.
fn add_to_whitelist(env: &Env, addr: Address) {
    env.storage().persistent().set(&DataKey::Whitelisted(addr), &true);
}
```

**Finding code:** [`S004`](error-codes.md) — flagged by the [`ledger_size`](detectors/ledger_size.md) detector.

---

### 12. Cap persistent collections — never let storage grow unbounded

**Rationale.** A persistent `Vec` or `Map` that grows on every call with no
removal path and no length cap is a storage-bloat vector. An attacker can call
the growth entrypoint in a loop until the entry exceeds the ledger size limit,
after which the contract's own writes start to fail. Either cap the length
explicitly or replace a single large collection with per-key entries that can be
pruned individually.

**Vulnerable**
```rust
#[contractimpl]
impl Registry {
    pub fn register(env: Env, who: Address) {
        let key = Symbol::new(&env, "members");
        let mut members: Vec<Address> = env.storage().persistent()
            .get(&key).unwrap_or(Vec::new(&env));
        // ❌ Grows forever — no cap, no removal.
        members.push_back(who);
        env.storage().persistent().set(&key, &members);
    }
}
```

**Fixed**
```rust
const MAX_MEMBERS: u32 = 500;

#[contractimpl]
impl Registry {
    pub fn register(env: Env, who: Address) -> Result<(), Error> {
        let key = Symbol::new(&env, "members");
        let mut members: Vec<Address> = env.storage().persistent()
            .get(&key).unwrap_or(Vec::new(&env));
        if members.len() >= MAX_MEMBERS {
            return Err(Error::RegistryFull); // ✅ Hard cap enforced.
        }
        members.push_back(who);
        env.storage().persistent().set(&key, &members);
        Ok(())
    }
}
```

**Finding code:** [`SANCT_UNBOUNDED_STORAGE`](error-codes.md) — flagged by the [`unbounded_storage`](detectors/unbounded_storage.md) detector.

---

## Theme 4 — Denial of Service

### 13. Cap Vec/Map arguments before iterating

**Rationale.** The Soroban host meters every instruction. A caller who passes an
oversized `Vec` or `Map` argument to an entrypoint that iterates it can exhaust
the resource budget, causing the call to revert. When the DoS'd function is on a
critical path — batch settlement, admin config, oracle update — this effectively
blocks the contract. Always enforce a maximum length before entering any loop
over a caller-supplied collection.

**Vulnerable**
```rust
#[contractimpl]
impl Payroll {
    // ❌ Caller controls payments.len() — an oversized Vec exhausts the budget.
    pub fn pay_all(env: Env, from: Address, payments: Vec<(Address, i128)>) {
        from.require_auth();
        for (to, amount) in payments.iter() {
            transfer(&env, &from, &to, amount);
        }
    }
}
```

**Fixed**
```rust
const MAX_BATCH: u32 = 100;

#[contractimpl]
impl Payroll {
    pub fn pay_all(env: Env, from: Address, payments: Vec<(Address, i128)>) -> Result<(), Error> {
        from.require_auth();
        // ✅ Reject before iteration — the budget stays bounded.
        if payments.len() > MAX_BATCH {
            return Err(Error::BatchTooLarge);
        }
        for (to, amount) in payments.iter() {
            transfer(&env, &from, &to, amount);
        }
        Ok(())
    }
}
```

**Finding code:** [`SANCT_ARG_DOS`](error-codes.md) — flagged by the [`arg_dos`](detectors/arg_dos.md) detector.

---

## Theme 5 — Panic Handling

### 14. Replace `panic!`/`unwrap`/`expect` with typed errors

**Rationale.** A `panic!`, `unwrap()`, or `expect()` in a contract entrypoint
aborts the entire transaction. When the panicking condition is attacker-reachable
— a missing storage entry, a failed parse, any fallible operation — an attacker
can reliably trigger it to DoS the contract. Use `Result<T, E>` with a typed
`#[contracterror]` enum so callers receive a structured error they can handle.

**Vulnerable**
```rust
// contracts/vulnerable-contract/src/lib.rs — reproduced for illustration
#[contractimpl]
impl VulnerableContract {
    pub fn fail_explicitly(_env: Env) {
        panic!("Something went wrong"); // ❌ Aborts every call unconditionally.
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        // ❌ unwrap() panics when the entry is missing (e.g. archived).
        env.storage().persistent().get(&id).unwrap()
    }
}
```

**Fixed**
```rust
#[contracterror]
#[derive(Copy, Clone)]
pub enum Error {
    NotFound = 1,
}

#[contractimpl]
impl Contract {
    pub fn balance(env: Env, id: Address) -> Result<i128, Error> {
        // ✅ Returns Err::NotFound instead of panicking on a missing entry.
        env.storage().persistent().get(&id).ok_or(Error::NotFound)
    }
}
```

**Finding code:** [`S002`](error-codes.md) / [`SANCT_UNWRAP`](error-codes.md) — flagged by the [`panic_detection`](detectors/panic_detection.md) and [`sanct_unwrap`](detectors/sanct_unwrap.md) detectors.

---

### 15. Propagate `Result` — never silently drop it

**Rationale.** Calling a fallible function and ignoring its `Result` is a latent
bug: the failure is invisible to the caller, state may be partially mutated, and
invariants can be silently violated. In Rust, `let _ = fallible()` and a
discarded return value both compile without warnings unless `#[must_use]` is
applied. In a financial contract, every error must be observed and handled.

**Vulnerable**
```rust
#[contractimpl]
impl Token {
    pub fn batch_transfer(env: Env, from: Address, recipients: Vec<Address>, amount: i128) {
        from.require_auth();
        for to in recipients.iter() {
            // ❌ Return value (and any error) is silently dropped.
            let _ = Self::transfer(env.clone(), from.clone(), to, amount);
        }
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    pub fn batch_transfer(
        env: Env, from: Address, recipients: Vec<Address>, amount: i128,
    ) -> Result<(), Error> {
        from.require_auth();
        for to in recipients.iter() {
            // ✅ Propagate — a single failure aborts the whole batch.
            Self::transfer(env.clone(), from.clone(), to, amount)?;
        }
        Ok(())
    }
}
```

**Finding code:** [`S009`](error-codes.md) — flagged by the [`unhandled_result`](detectors/unhandled_result.md) detector.

---

### 16. View/getter entrypoints must not panic or write state

**Rationale.** Functions named `get_*`, `balance`, `query_*`, or similar are
expected by callers to be read-only and infallible. A reachable `panic!` in a
getter lets an attacker turn any balance query into a DoS. A storage write
inside a view function violates the read-only contract, surprises callers, and
can create subtle state inconsistencies.

**Vulnerable**
```rust
#[contractimpl]
impl Token {
    pub fn get_balance(env: Env, id: Address) -> i128 {
        // ❌ Panics when entry is absent — attackable DoS on every balance query.
        env.storage().persistent().get(&id).expect("no balance")
    }

    pub fn query_config(env: Env) -> Config {
        let cfg: Config = env.storage().instance().get(&DataKey::Config).unwrap();
        // ❌ State write inside a view function.
        env.storage().instance().set(&DataKey::LastQueried, &env.ledger().sequence());
        cfg
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    pub fn get_balance(env: Env, id: Address) -> i128 {
        // ✅ Returns 0 for unknown accounts instead of panicking.
        env.storage().persistent().get(&id).unwrap_or(0)
    }

    pub fn query_config(env: Env) -> Result<Config, Error> {
        // ✅ Pure read — no side effects.
        env.storage().instance().get(&DataKey::Config).ok_or(Error::NotInitialised)
    }
}
```

**Finding codes:** [`SANCT_VIEW_PANIC`](error-codes.md) and [`SANCT_STATE_WRITE_IN_VIEW`](error-codes.md) — flagged by the [`view_panic`](detectors/view_panic.md) and [`state_write_in_view`](detectors/state_write_in_view.md) detectors.

---

## Theme 6 — Authorization Patterns

### 17. Use delta or compare-and-set semantics for `approve`

**Rationale.** An `approve` that blindly overwrites the stored allowance is
vulnerable to a front-running race (ERC-20 TOCTOU): a spender watching for the
transaction can spend the old allowance `N` before the update lands, then spend
the new allowance `M`, drawing `N + M` against an owner who only intended one.
Use `increase_allowance`/`decrease_allowance` deltas or require the caller to
supply the expected current value (compare-and-set).

**Vulnerable**
```rust
// contracts/token-with-bugs/src/lib.rs — reproduced for illustration
#[contractimpl]
impl Token {
    pub fn approve(e: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        // ❌ Unconditional overwrite — the classic approve front-running race.
        e.storage().persistent().set(&(owner, spender), &amount);
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    // ✅ Delta semantics — no race because the change is relative.
    pub fn increase_allowance(e: Env, owner: Address, spender: Address, delta: i128) {
        owner.require_auth();
        let key = (owner, spender);
        let current: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        e.storage().persistent().set(&key, &(current + delta));
    }

    pub fn decrease_allowance(e: Env, owner: Address, spender: Address, delta: i128) {
        owner.require_auth();
        let key = (owner, spender);
        let current: i128 = e.storage().persistent().get(&key).unwrap_or(0);
        let new_val = current.checked_sub(delta).unwrap_or(0);
        e.storage().persistent().set(&key, &new_val);
    }
}
```

**Finding code:** [`SANCT_ALLOWANCE_RACE`](error-codes.md) — flagged by the [`allowance_race`](detectors/allowance_race.md) detector.

---

## Theme 7 — Input Validation

### 18. Validate edge amounts: reject zero and self-transfers

**Rationale.** A `transfer` or `mint` that accepts `amount = 0` or `from == to`
silently completes without moving value but still emits events and consumes
budget. This can confuse downstream off-chain indexers, trigger erroneous
notifications, and create edge cases in invariant proofs. Guard both conditions
at the top of every amount-handling function.

**Vulnerable**
```rust
#[contractimpl]
impl Token {
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        // ❌ Allows zero-value transfers and self-transfers.
        let bal: i128 = e.storage().persistent().get(&from).unwrap_or(0);
        e.storage().persistent().set(&from, &(bal - amount));
        let to_bal: i128 = e.storage().persistent().get(&to).unwrap_or(0);
        e.storage().persistent().set(&to, &(to_bal + amount));
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Token {
    pub fn transfer(e: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {
        from.require_auth();
        // ✅ Reject nonsensical inputs before touching storage.
        if amount <= 0 { return Err(Error::InvalidAmount); }
        if from == to  { return Err(Error::SelfTransfer); }
        let bal: i128 = e.storage().persistent().get(&from).unwrap_or(0);
        e.storage().persistent().set(&from, &(bal - amount));
        let to_bal: i128 = e.storage().persistent().get(&to).unwrap_or(0);
        e.storage().persistent().set(&to, &(to_bal + amount));
        Ok(())
    }
}
```

**Finding code:** [`S013`](error-codes.md) — flagged by the [`edge_amount`](detectors/edge_amount.md) detector.

---

### 19. Compare balances with `>=`/`<=`, not `==`/`!=`

**Rationale.** Gating a transfer on an exact balance equality (`balance == threshold`)
almost never triggers as intended: any rounding, fee deduction, or concurrent
deposit shifts the balance away from the exact value. Use relational comparisons
(`>=`, `<=`) for threshold checks, and reserve equality only for zero/non-zero
sentinels where exact equality is guaranteed.

**Vulnerable**
```rust
pub fn can_withdraw(balance: i128, min_required: i128) -> bool {
    // ❌ Only true when balance is *exactly* min_required — usually never.
    balance == min_required
}
```

**Fixed**
```rust
pub fn can_withdraw(balance: i128, min_required: i128) -> bool {
    // ✅ True whenever the account has at least the required amount.
    balance >= min_required
}
```

**Finding code:** [`SANCT_BALANCE_EQ`](error-codes.md) — flagged by the [`balance_equality`](detectors/balance_equality.md) detector.

---

## Theme 8 — Time Logic

### 20. Use `timestamp()` for real-time windows, not `sequence()`

**Rationale.** `env.ledger().sequence()` is a block counter that advances roughly
once every ~5 seconds. Adding a seconds-magnitude literal to it — such as
`sequence() + 86_400` to mean "one day" — produces a deadline ~5 days in the
future, not one day. Real-time durations (deadlines, lock periods, vesting
schedules) belong with `env.ledger().timestamp()`, which is measured in
seconds since the Unix epoch. Use `sequence()` only for relative ledger-count
windows (e.g. TTL bumps).

**Vulnerable**
```rust
#[contractimpl]
impl Escrow {
    pub fn lock_until(env: Env) -> u32 {
        // ❌ Adds 86,400 *ledgers* (~5 days) instead of 86,400 *seconds* (1 day).
        env.ledger().sequence() + 86_400
    }
}
```

**Fixed**
```rust
#[contractimpl]
impl Escrow {
    pub fn lock_until(env: Env) -> u64 {
        // ✅ timestamp() is seconds — adding 86,400 really means one day.
        env.ledger().timestamp() + 86_400
    }
}
```

**Finding code:** [`S021`](error-codes.md) — flagged by the [`ledger_seconds`](detectors/ledger_seconds.md) detector.

---

## See Also

- [Finding Code Catalog](error-codes.md) — stable codes emitted by every detector.
- [Detector Catalog](detectors/README.md) — deep-dive pages for each rule.
- [Glossary](glossary.md) — 50+ Soroban/Stellar security terms.
- [Case Study: Admin Takeover via Missing `require_auth`](case-studies/admin-takeover.md)
- [Awesome Soroban Security](awesome-soroban-security.md) — external resources.
- [Detector Cookbook](detector-cookbook.md) — how to write a new Sanctifier detector.

---

*Last updated: 2026-07-26 · Maintained by the [Sanctifier](https://github.com/Centurylong/sanctifier) project.*
