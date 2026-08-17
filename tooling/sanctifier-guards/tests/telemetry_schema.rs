//! Executable counterpart to `docs/telemetry-schema.md`. Exercises a real
//! guard trip end-to-end and asserts the event shape matches exactly what
//! the schema doc promises monitors: a single-topic `(Symbol,)` tuple with
//! value `inv_fail`, and a `(Symbol, String)` data payload where the symbol
//! is always `cond`. If a future change to the guard macros alters the
//! topic, the payload symbol, or the tuple arity, this test — not just a
//! monitor in production — is what catches it.

use sanctifier_guards::guard_invariant_result;
use soroban_sdk::{
    contract, contracterror, contractimpl, symbol_short, testutils::Events, Env, Error, Symbol,
    TryFromVal, Val,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SchemaDemoError {
    Tripped = 1,
}

#[contract]
pub struct SchemaDemo;

#[contractimpl]
impl SchemaDemo {
    pub fn trip(env: Env) -> Result<(), Error> {
        guard_invariant_result!(&env, 1 == 2, SchemaDemoError::Tripped);
        Ok(())
    }
}

#[test]
fn guard_trip_event_matches_documented_schema() {
    let env = Env::default();
    let id = env.register_contract(None, SchemaDemo);
    let client = SchemaDemoClient::new(&env, &id);

    let _ = client.try_trip();

    let events = env.events().all();
    let mut found = false;
    for (_contract_id, topics, data) in events.iter() {
        // Topics: a single-element tuple `(Symbol,)`.
        assert_eq!(topics.len(), 1, "schema requires exactly one topic");
        let topic_sym = Symbol::try_from_val(&env, &topics.first().unwrap())
            .expect("topic must decode as a Symbol");
        if topic_sym != Symbol::new(&env, "inv_fail") {
            continue;
        }
        found = true;

        // Data payload: a two-element tuple `(Symbol, String)`, first
        // element always `symbol_short!("cond")` per the schema doc.
        let (data_sym, _message): (Symbol, soroban_sdk::String) =
            TryFromVal::try_from_val(&env, &data).expect("payload must decode as (Symbol, String)");
        assert_eq!(
            data_sym,
            symbol_short!("cond"),
            "data payload's first element must always be the `cond` symbol"
        );
    }

    assert!(found, "no inv_fail event matched the documented schema");

    // Cross-check against the exported topic constant so the doc, the
    // macro, and the constant can never silently drift from each other.
    assert_eq!(sanctifier_guards::INVARIANT_FAILURE_TOPIC, "inv_fail");

    // Reference to keep the import list honest if TryFromVal's blanket impl
    // for tuples is ever resolved differently.
    let _: fn(&Env, &Val) = |_e, _v| {};
}
