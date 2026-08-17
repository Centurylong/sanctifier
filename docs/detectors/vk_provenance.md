# `vk_provenance` — Unpinned/unverified ZK verifying key

| | |
| --- | --- |
| **Finding code** | [`SANCT_VK_PROVENANCE`](../error-codes.md) |
| **Category** | cryptography |
| **Severity** | High |
| **Source rule** | [`rules/vk_provenance.rs`](../../tooling/sanctifier-core/src/rules/vk_provenance.rs) |
| **Glossary** | [Verifying key](../glossary.md#verifying-key) |

## What it catches

A ZK verifier contract that accepts its Groth16/Plonk **verifying key** (VK)
as a runtime parameter and stores it with no provenance check. If the VK
isn't pinned — checked against a committed hash, gated behind
`require_auth()` from a fixed admin, or simply hardcoded at compile time —
whoever calls the setter controls which key proofs are checked against. An
attacker can supply their own trusted-setup key and submit proofs that verify
perfectly against *that* key, bypassing the circuit's real constraints
entirely. This is a silent, complete soundness break: `verify()` keeps
returning `true`, just for the wrong statement.

## Vulnerable example

This is the actual shape of `contracts/zk-verifier/src/lib.rs` in this repo
before this detector existed:

```rust
#[contractimpl]
impl ZkVerifierContract {
    pub fn init(env: Env, vk_bytes: Bytes) {
        if env.storage().instance().has(&DataKey::VerifyingKey) {
            panic!("Already initialized");
        }
        // No require_auth(), no hash check against a committed VK — the
        // first caller to invoke init() decides which key every future
        // proof is checked against.
        env.storage()
            .instance()
            .set(&DataKey::VerifyingKey, &vk_bytes);
    }
}
```

Because `init()` has no authorization gate, this isn't even limited to a
malicious *deployer* — on a public network, anyone who front-runs the real
`init()` call controls the trusted key going forward.

## The fix

Pick one of two recognized-safe patterns:

**Gate the setter** with `require_auth()` from a fixed admin *and* pin the
key with a hash check, so only a specific, already-trusted account can ever
change it, and only to a pre-committed key:

```rust
pub fn init(env: Env, admin: Address, vk_bytes: Bytes) {
    admin.require_auth();
    let digest = env.crypto().sha256(&vk_bytes);
    assert_eq!(digest.to_bytes(), COMMITTED_VK_HASH, "unexpected verifying key");
    env.storage().instance().set(&DataKey::VerifyingKey, &vk_bytes);
}
```

**Or hardcode the VK at compile time** — the strongest option, since it
removes the runtime attack surface entirely:

```rust
const VK_BYTES: [u8; 192] = [ /* trusted-setup output, committed to source control */ ];

pub fn init(env: Env) {
    let vk = Bytes::from_array(&env, &VK_BYTES);
    env.storage().instance().set(&DataKey::VerifyingKey, &vk);
}
```

## How Sanctifier detects it

The rule looks for a function parameter whose name suggests a verifying key
(`vk`, `vk_bytes`, `verifying_key`, …) and whose type mentions `Bytes` /
`BytesN<N>`. If that parameter is written into contract storage under a
VK-shaped key (`env.storage()....set(&DataKey::VerifyingKey, &vk_bytes)` or
similar) and the enclosing function contains neither a `require_auth()` call
nor a hash-comparison guard (`assert_eq!`/`assert_ne!`/`==`/`!=` alongside a
`hash`-named value), it's flagged. A VK that's hardcoded as a source-level
constant — never accepted as a parameter at all — is the recognized-safe
pattern and is never flagged.

**Limitations:** this is a structural/textual heuristic, not a data-flow
analysis — a hash check anywhere in the same function suppresses the finding
even if it doesn't actually gate the `.set(...)` call, and a guard implemented
in a helper function it calls out to won't be seen. Treat a clean report as
"no *obvious* unguarded VK acceptance," not a soundness proof.

## References

- [ZK Verification epic — issue #736](https://github.com/Centurylong/sanctifier/issues/736)
- [CWE-345: Insufficient Verification of Data Authenticity](https://cwe.mitre.org/data/definitions/345.html)
- Related: [`hardcoded_addr`](hardcoded_addr.md) (the inverse pattern — there,
  hardcoding is the *vulnerability*; here, hardcoding the VK is the *fix*)
