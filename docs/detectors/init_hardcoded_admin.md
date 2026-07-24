# init_hardcoded_admin

* **Finding Code:** `SANCT_INIT_HARDCODED_ADMIN`
* **Category:** Authentication
* **Severity:** Warning / Error

## Description

Detects hardcoded admin addresses, secret seeds, hex hashes, or default byte arrays inside contract initialization functions (`initialize`, `init`, `reinitialize`, etc.).

Initialization functions should accept the administrator address as a formal argument (`admin: Address`) rather than hardcoding address literals or placeholder values.

## Vulnerable Example

```rust
use soroban_sdk::{contractimpl, Env};

#[contractimpl]
impl Contract {
    pub fn initialize(env: Env) {
        let admin = "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVSGZ";
        env.storage().instance().set(&"admin", &admin);
    }
}
```

## Fixed Example

```rust
use soroban_sdk::{contractimpl, Address, Env};

#[contractimpl]
impl Contract {
    pub fn initialize(env: Env, admin: Address) {
        admin.require_auth();
        env.storage().instance().set(&"admin", &admin);
    }
}
```

## Remediation

Require the admin address as a formal parameter (`admin: Address`) during initialization and authorize the caller using `admin.require_auth()`.
