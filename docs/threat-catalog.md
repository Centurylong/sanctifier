# Threat Catalog: Finding Codes → Real-World Impact

Every code in [Finding Codes](error-codes.md) maps to a class of bug that has,
in one form or another, actually cost money or availability on a live
contract system. This catalog exists so a triager can answer *"why does this
finding matter?"* in one line, and so `priority:*` labeling reflects real
blast radius rather than gut feel.

Stellar Soroban is a newer runtime than EVM, so its own public incident
history is thin; where no Soroban-native incident is public, the reference is
the closest **EVM-ecosystem analog** for the same *vulnerability class*
(marked accordingly) — the underlying bug pattern (missing auth, unchecked
arithmetic, unbounded iteration, ...) is runtime-agnostic even though the
bytecode isn't. This mirrors the methodology already used in
[`differential-testing.md`](differential-testing.md) for cross-tool
comparison.

## Authentication & authorization

| Code | Detector | Real-world impact | References |
| --- | --- | --- | --- |
| `S001` | [`auth_gap`](detectors/auth_gap.md) | A state-mutating entrypoint missing `require_auth` lets *any* account call it — direct theft or arbitrary state corruption, no exploit sophistication required. | [CWE-306](https://cwe.mitre.org/data/definitions/306.html); analog: [Parity multisig wallet — anyone-can-call `initWallet`](https://www.parity.io/a-postmortem-on-the-parity-multi-sig-library-self-destruct/) (EVM, 2017, ~$280M frozen) |
| `S024` | [`wrong_auth_args`](detectors/wrong_auth_args.md) | `require_auth()` instead of `require_auth_for_args()` authenticates *that a signer signed something*, not *that they signed this specific call's arguments* — a valid signature for one invocation can be replayed against different arguments. | [CWE-347](https://cwe.mitre.org/data/definitions/347.html) (improper verification of cryptographic signature); [Soroban auth docs](https://developers.stellar.org/docs/build/guides/auth) |
| `SANCT_VISIBILITY` | [`sanct_visibility`](detectors/sanct_visibility.md) | A helper function shaped like an internal state mutator (naming/signature pattern) is exported publicly with no auth guard — the "S001 that hides in a refactor," typically introduced when a private helper is promoted to `pub` for reuse. | [CWE-284](https://cwe.mitre.org/data/definitions/284.html) |
| `SANCT_INIT_HARDCODED_ADMIN` | [`init_hardcoded_admin`](detectors/init_hardcoded_admin.md) | Hardcoding the admin address at `initialize()` instead of taking it as an argument means every deployment shares one admin key (or the deployer's placeholder), and the contract can't be redeployed to a different environment (testnet/mainnet) without a source edit. | [CWE-798](https://cwe.mitre.org/data/definitions/798.html) |
| `S023` | [`reserve_withdrawal`](detectors/reserve_withdrawal.md) | Missing strict authorization on a reserve/treasury withdrawal path — the highest blast-radius auth gap, since it targets pooled funds rather than a single user's balance. | Analog: [Cream Finance — access-control gap in a lending pool, $130M](https://rekt.news/cream-rekt-2/) (EVM, 2021) |
| `SANCT_ALLOWANCE_RACE` | [`allowance_race`](detectors/allowance_race.md) | `approve` overwriting an allowance unconditionally lets a spender front-run an approval change to spend both the old *and* new allowance. | [SWC-114](https://swcregistry.io/docs/SWC-114) (transaction order dependence); the canonical ERC-20 approve race |

## Arithmetic

| Code | Detector | Real-world impact | References |
| --- | --- | --- | --- |
| `S003` | [`arithmetic_overflow`](detectors/arithmetic_overflow.md) | Unchecked `+`/`-`/`*` that overflows wraps silently in release builds without `overflow-checks`, turning "add 1" into "subtract u64::MAX" — the classic integer-overflow mint/burn exploit shape. | [CWE-190](https://cwe.mitre.org/data/definitions/190.html); analog: [BeautyChain (BEC) — overflow mint of unlimited tokens](https://blog.peckshield.com/2018/04/22/beauty/) (EVM, 2018) |
| `S019` | [`unsigned_underflow`](detectors/unsigned_underflow.md) | Unsigned subtraction wrapping past zero produces a near-`MAX` value instead of panicking pre-checked-arithmetic-era — e.g. `balance - amount` when `amount > balance` yields a huge balance instead of an error. | [CWE-191](https://cwe.mitre.org/data/definitions/191.html) |
| `S018` | [`division_by_zero`](detectors/division_by_zero.md) | Division/modulo by an unproven-nonzero value panics the whole invocation on-chain — a caller-triggerable denial of service on that entrypoint (e.g. an empty pool's `total_supply` as a divisor). | [CWE-369](https://cwe.mitre.org/data/definitions/369.html) |
| `S017` | [`fee_rounding`](detectors/fee_rounding.md) | Integer-division fee calculation rounding to zero for micro-amounts lets an attacker structure many small transactions to evade fees entirely — a slow drain via volume rather than a single exploit. | [CWE-682](https://cwe.mitre.org/data/definitions/682.html) (incorrect calculation) |
| `S021` | [`ledger_seconds`](detectors/ledger_seconds.md) | Mixing a ledger sequence number (block counter, increments ~1/5s) with a seconds-magnitude literal produces a time window off by roughly the ledger-to-second ratio — a lock/vesting/auction window that's wildly shorter or longer than intended. | [Soroban ledger close time](https://developers.stellar.org/docs/learn/fundamentals/stellar-consensus-protocol) |
| `SANCT_SHIFT_OVERFLOW` | [`shift_overflow`](detectors/shift_overflow.md) | A bit-shift amount `>=` the operand's bit width is undefined behavior in release builds — a corrupted packed value or an abort an attacker can trigger with an out-of-range shift argument. | [CWE-1335](https://cwe.mitre.org/data/definitions/1335.html) |
| `S022` | [`tier_boundary_off_by_one`](detectors/tier_boundary_off_by_one.md) | Mixing strict/inclusive comparisons in the same tier ladder (e.g. `< 100` then `<= 100`) misassigns the boundary value's rank — usually a fee-tier or access-tier miscalculation at the exact threshold, silent and hard to notice in review. | [CWE-193](https://cwe.mitre.org/data/definitions/193.html) (off-by-one) |

## Panics & denial of service

| Code | Detector | Real-world impact | References |
| --- | --- | --- | --- |
| `S002` | [`panic_detection`](detectors/panic_detection.md) | `panic!`/`unwrap`/`expect` in a reachable path aborts the *entire* invocation — for a batch/multi-step operation, one bad element denies service to every other element in the same call. | [CWE-248](https://cwe.mitre.org/data/definitions/248.html) (uncaught exception) |
| `SANCT_UNWRAP` | [`sanct_unwrap`](detectors/sanct_unwrap.md) | Same failure mode as `S002`, scoped specifically to `#[contractimpl]` entrypoints — the highest-value place to catch it, since these are directly attacker-reachable. | [CWE-248](https://cwe.mitre.org/data/definitions/248.html) |
| `SANCT_VIEW_PANIC` | [`view_panic`](detectors/view_panic.md) | A panic reachable from a *view/getter* is worse than a mutator panicking — callers (including other contracts and off-chain indexers) assume reads are always safe and don't guard against them, so one bad stored value can take down every read-path integrator. | [CWE-248](https://cwe.mitre.org/data/definitions/248.html) |
| `SANCT_ARG_DOS` | [`arg_dos`](detectors/arg_dos.md) | Iterating a caller-supplied `Vec`/`Map` argument with no length cap lets an attacker submit an oversized collection to exhaust the transaction's CPU/memory budget — a resource-exhaustion DoS paid for (cheaply) by the attacker, not the protocol. | [CWE-405](https://cwe.mitre.org/data/definitions/405.html) (asymmetric resource consumption); analog: [Solidity unbounded-array-iteration gas-limit DoS](https://swcregistry.io/docs/SWC-128) |
| `SANCT_UNBOUNDED_STORAGE` | [`unbounded_storage`](detectors/unbounded_storage.md) | A persistent/instance collection that only grows (append/insert, never removed or capped) eventually exceeds the ledger entry size limit, bricking every function that touches it — a slow-motion DoS baked in at design time rather than triggered by one attacker action. | Analog: unbounded on-chain array growth patterns flagged by [Slither's `array-length-outside-loop`/`costly-loop` class](https://github.com/crytic/slither/wiki/Detector-Documentation) |
| `SANCT_UNBOUNDED_RETURN` | [`unbounded_return`](detectors/unbounded_return.md) | A public entrypoint returning an unbounded `Vec`/`Map` risks the response exceeding the host's resource limits as the underlying collection grows — a function that worked in testing "goes down" months later purely from data growth, with no code change. | [CWE-789](https://cwe.mitre.org/data/definitions/789.html) (uncontrolled memory allocation) |
| `S004` | [`ledger_size`](detectors/ledger_size.md) | A `contracttype` layout approaching the ledger entry size limit risks writes starting to fail once real data fills in the fields that were empty/small during testing. | [Soroban state size limits](https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/state-archival) |
| `S006` | [`missing_ttl`](detectors/missing_ttl.md) | Persistent/instance storage accessed without extending its TTL can be archived (evicted) by the network, and the next read fails until it's explicitly restored — an availability bug that only surfaces after the entry's TTL window has already elapsed in production. | [Soroban state archival](https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/state-archival) |

## Logic & code hygiene

| Code | Detector | Real-world impact | References |
| --- | --- | --- | --- |
| `S009` | [`unhandled_result`](detectors/unhandled_result.md) | A silently-dropped `Result` means a failed operation (e.g. a failed transfer) is treated as if it succeeded — state and reality diverge, usually discovered only when a downstream invariant breaks. | [CWE-252](https://cwe.mitre.org/data/definitions/252.html) (unchecked return value) |
| `SANCT_BALANCE_EQ` | [`balance_equality`](detectors/balance_equality.md) | Gating logic on `balance == threshold` instead of `>=`/`<=` is bypassed entirely once the balance overshoots the exact value (e.g. via a second deposit) — the check silently stops firing rather than erroring. | [CWE-697](https://cwe.mitre.org/data/definitions/697.html) (incorrect comparison) |
| `SANCT_STATE_WRITE_IN_VIEW` | [`state_write_in_view`](detectors/state_write_in_view.md) | A getter/view-named function performing a storage write violates the read-only contract callers (and static analyzers) assume of anything named `get_*`/`view_*` — a caller batching "safe" reads can trigger unexpected state changes. | [CWE-841](https://cwe.mitre.org/data/definitions/841.html) (improper enforcement of behavioral workflow) |
| `S013` | [`edge_amount`](detectors/edge_amount.md) | `transfer`/`mint`/`burn` missing `amount > 0` or `from != to` guards allows zero-value spam transactions (event-log pollution, indexer noise) or self-transfers that can confuse balance accounting depending on operation order. | [CWE-1284](https://cwe.mitre.org/data/definitions/1284.html) (improper validation of specified quantity) |
| `SANCT_CONTRACTERROR_ENUM` | [`contracterror_enum`](detectors/contracterror_enum.md) | A public function returning an error enum missing `#[contracterror]`/explicit `repr` risks unstable or colliding discriminants across compiler versions — callers matching on the numeric error code silently misinterpret the failure reason. | [Soroban `#[contracterror]` docs](https://developers.stellar.org/docs/build/smart-contracts/example-contracts/errors) |
| `S016` | [`error_code_collision`](detectors/error_code_collision.md) | Duplicate/inconsistent discriminants in a `#[contracterror]` enum mean two logically distinct failures surface the same numeric code to callers — error-handling logic (retry vs. abort) branches on the wrong signal. | [CWE-1164](https://cwe.mitre.org/data/definitions/1164.html) (irrelevant code, adjacent to ambiguous error semantics) |
| `S012` | [`hardcoded_addr`](detectors/hardcoded_addr.md) | A hardcoded address/secret literal used in an auth context can't be rotated without redeploying the contract, and if it's a placeholder/test value that shipped to production, it's a known, public credential. | [CWE-798](https://cwe.mitre.org/data/definitions/798.html) |
| `S015` | [`unused_variable`](detectors/unused_variable.md) | Lower severity than the rest of this catalog on its own, but an unused binding is frequently the fossil of an intended check that was written and never wired in (e.g. a validation result computed, then never matched on) — worth a second look at the diff, not just the line. | [CWE-563](https://cwe.mitre.org/data/definitions/563.html) |
| `S020` | [`excessive_clone`](detectors/excessive_clone.md) | Cloning the `Env` handle where `&env` suffices burns gas on every call — not a security bug, but a cost one: at scale it's a measurable per-transaction fee tax with no functional benefit. | Soroban resource-fee model — cost scales with instructions executed |
| `SANCT_EAGER_UNWRAP_OR` | [`eager_unwrap_or`](detectors/eager_unwrap_or.md) | `.unwrap_or(expensive_call())` always evaluates the fallback eagerly, even on the success path — a hidden gas cost paid on every call regardless of whether the fallback is ever used; `.unwrap_or_else(|| ...)` defers it. | Rust API guidelines — prefer `unwrap_or_else` for non-trivial defaults |

## Reserved codes without a live detector yet

These finding codes are defined in `finding_codes.rs` for the CLI/formal-verification pipeline but aren't (yet) backed by a registered `Rule` — listed for completeness so the catalog stays a full map of the code space, not just the enforced subset.

| Code | Category | Real-world impact | References |
| --- | --- | --- | --- |
| `S005` | storage_keys | A storage key collision across two logically distinct data paths lets a write to one silently corrupt the other — same failure shape as a hash-map key collision, but persistent and hard to detect without a targeted test. | [CWE-694](https://cwe.mitre.org/data/definitions/694.html) |
| `S007` | custom_rule | User-defined rule matches are project-specific by definition; impact depends entirely on the custom rule's intent. | — |
| `S008` | events | Inconsistent event topic counts/shapes break off-chain indexers that pattern-match on topic structure — an integration-layer outage rather than an on-chain one. | [Soroban events](https://developers.stellar.org/docs/build/guides/events) |
| `S010` | upgrades | A security gap in contract upgrade/admin mechanisms is a "keys to the kingdom" bug class — a flawed upgrade path can replace the entire contract logic. | Analog: [Wormhole — signature verification bypass in a bridge upgrade path, $325M](https://rekt.news/wormhole-rekt/) (Solana/EVM bridge, 2022) |
| `S011` | formal_verification | An SMT (Z3) invariant violation is a mathematical proof that a stated property (e.g. `x * y >= k` for an AMM) can be broken — the highest-confidence finding class in the tool, since it isn't a heuristic. | [`docs/formal-verification-video-series.md`](formal-verification-video-series.md) |
| `S014` | code_hygiene | A deprecated `soroban-sdk` host function may be removed in a future SDK version, silently breaking the build (or worse, changing behavior) on the next upgrade. | `soroban-sdk` changelog |
| `W001`–`W004` | wasm | Compiled-module checks (`sanctifier wasm`): a module missing the Soroban contract spec, exporting nothing callable, missing environment metadata, or using float types the host rejects — each is a *deploy-time* failure caught before spending a transaction fee to find out on-chain. | [Soroban WASM ABI](https://developers.stellar.org/docs/learn/fundamentals/contract-development/types/) |

## See also

- [Finding Codes](error-codes.md) — the code → detector-page index this catalog expands on
- [Detector Catalog](detectors/README.md) — per-detector pages (what it catches, vulnerable example, fix)
- [Differential Testing](differential-testing.md) — how Sanctifier's coverage compares to Slither/Aderyn for the same vulnerability classes
