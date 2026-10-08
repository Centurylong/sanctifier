# CEI / token-callback reentrancy detector

**Finding code:** SANCT_CEI (high / Error)

**Related funded issues:** #336 (shared CEI ordering model), #310 (reentrancy
finding), and #325 (SAC/token callbacks).

The rule flags a Soroban contract method when a recognizable **cross-contract
interaction** executes before a **storage write** on the *same syntactic path*.
External calls are reentrancy boundaries: a token or other contract may call
back while a balance or other invariant still describes the old state.

## Example: interaction before effect

~~~rust
pub fn withdraw(e: Env, account: Address, token: Address, amount: i128) {
    account.require_auth();
    let t = token::Client::new(&e, &token);
    t.transfer(&e.current_contract_address(), &account, &amount);
    e.storage().persistent().set(&account, &new_balance); // SANCT_CEI
}
~~~

**Remediation:** validate and authorize first; update internal state before
calling the token client, and use reentrancy protection where appropriate.
On Soroban, an abort rolls back all contract operations in the transaction;
verify that the intended invariants and failure handling remain correct.

~~~rust
pub fn withdraw(e: Env, account: Address, token: Address, amount: i128) {
    account.require_auth();
    e.storage().persistent().set(&account, &new_balance);
    let t = token::Client::new(&e, &token);
    t.transfer(&e.current_contract_address(), &account, &amount);
}
~~~

## Interaction / effect classification

- **Checks:** calls to require_auth or require_auth_for_args.
- **Effects:** set, update or remove on persistent/instance/temporary storage;
  a bound local storage handle; and common explicit write_balance/write_state
  helpers.
- **Interactions:** Env invoke_contract / try_invoke_contract, calls through
  generated TokenClient / token::Client / SAC-style clients, including transfer,
  transfer_from, burn, mint and other generated client entrypoints.

The shared API, exposed as sanctifier_core::reentrancy::models(source), produces
one CeiModel per free or inherent function, with path-local ordered CeiEvent
records. Models expose effects(), interactions(), and
interactions_before_effects(). The default RuleRegistry registers
cei_violation, so regular source analysis surfaces SANCT_CEI automatically.

## Scope and limitations

This is a **conservative static warning**, not a verification of all reachable
control-flow paths or absence of reentrancy. It distinguishes if/else and match
arms to avoid joining mutually exclusive calls and writes. Loop bodies are
modeled with zero/one iterations and loops_present is recorded; nested closures,
unrecognized storage aliases, dynamic dispatch, cross-function effects, and
calls hidden behind complex helper abstractions are not followed. Path counts
are capped to bound worst-case analysis; that may produce false negatives.
Review findings against the real call graph and invariants before remediation.

The focused regression cases live alongside cei_violation.rs, including
token::Client transfer and transfer_from, host invoke_contract, write-before-
interaction safety, mutually exclusive branches, a local storage handle and
loop labeling.
The golden snapshot fixture
`tooling/sanctifier-core/tests/fixtures/detectors/cei_violation.rs` pairs
vulnerable SAC transfer, transfer_from and invoke_contract entrypoints with
safe write-first, exclusive-branch and event-only counterparts.
