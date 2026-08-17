# Guard trip telemetry schema

`sanctifier-guards` publishes a Soroban contract event every time a guard
trips (or, optionally, passes). This document is the stable contract for
anyone building a monitor, indexer, or alert rule against those events —
it exists so "what does an `inv_fail` event look like" has one authoritative
answer instead of living only in macro source comments.

## Why this matters

A guard trip (an invariant violation, a rejected owner/role/timelock check,
or an overflow caught by a math guard) is a security-relevant signal. The
whole point of publishing an event *before* trapping the transaction is that
the event survives the subsequent rollback — Soroban commits events into the
transaction's event set even though `panic_with_error!` reverts contract
storage. A monitor subscribed to the schema below sees every guard trip
across every contract that uses this crate, in real time, even though each
individual transaction that trips a guard ultimately fails.

## Event shape

Every event published by this crate has exactly this shape:

| Field | Value |
| --- | --- |
| Contract | The Soroban contract `Address` that invoked the guard (standard event field, not guard-specific). |
| Topics | A single-element tuple: `(Symbol,)`. |
| Topic value | `symbol_short!("inv_fail")` on a guard trip, `symbol_short!("inv_pass")` on a recorded pass. Both fit the 9-byte `symbol_short!` budget (8 bytes each). |
| Data payload | A two-element tuple: `(Symbol, String)`. |
| Data\[0\] | `symbol_short!("cond")` — always this exact symbol, regardless of which guard macro produced the event. |
| Data\[1\] | A `soroban_sdk::String` holding a human-readable description of what was checked. For `guard_invariant!` and the access-control guards, this is `stringify!($cond)` — the literal Rust source of the condition expression. For the math guards, this is a fixed message such as `"checked_add overflow"`. |

### Why one topic, not one topic per guard kind

Topics are a scarce, indexed Soroban resource. Every guard in this crate —
`guard_invariant!`, `guard_owner!`, `guard_role!`, `guard_timelock_elapsed!`,
`guard_checked_add!`/`sub!`/`mul!`/`div!`, and their `_result` variants —
publishes to the *same* `inv_fail` topic on failure. A monitor subscribes
once and sees every guard kind; it distinguishes *which* guard tripped by
reading the data payload's message, not by juggling a growing list of
topics per guard type. This also means adding a new guard macro to this
crate is never a breaking change to existing monitor subscriptions.

## Subscribing as a monitor

Any tool that reads Soroban contract events (`getEvents` on Soroban RPC,
Horizon's effects/operations for the deploying account, or a custom
indexer) should filter on:

- `topics[0] == inv_fail` (guard trip) or `topics[0] == inv_pass` (recorded
  pass, only emitted by `guard_invariant_pass!`).
- Then decode `data[1]` as a UTF-8 string for the human-readable condition
  or failure message.

Because the event is committed before the trap, it is present in the
transaction's event set *even for a transaction whose overall result is a
failure* (`panic_with_error!` reverts state, not the already-emitted
event). Monitors must not filter out failed transactions when scanning for
`inv_fail` — that would miss every guard trip, which is the one thing this
schema exists to make visible.

## Stability

`INVARIANT_FAILURE_TOPIC` (`"inv_fail"`) and `INVARIANT_PASS_TOPIC`
(`"inv_pass"`) are exported as `pub const` from `sanctifier_guards` so
consumers never need to hardcode the string literals. A compile-time test
in the crate (`topic_constants_fit_symbol_short_budget`) guarantees both
stay within the 9-byte `symbol_short!` budget, so a future rename that
would silently break this schema fails to compile instead of shipping.
`tests/telemetry_schema.rs` in this crate exercises the schema end-to-end
against a real guard trip and asserts the exact shape documented above —
if a future change to `guard_invariant!` altered the topic, payload
symbol, or tuple arity, that test fails.
