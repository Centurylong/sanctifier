#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Vec};

// FIXTURE: unbounded_event_emission detector
// Publishing an event from inside a loop scales the event count with the
// loop's iteration count.

#[contract]
pub struct UnboundedEventEmissionContract;

#[contractimpl]
impl UnboundedEventEmissionContract {
    // Violation: publish called once per loop iteration.
    pub fn notify_all(env: Env, recipients: Vec<Address>) {
        for to in recipients.iter() {
            env.events().publish((symbol_short!("paid"), to.clone()), 1_i128);
        }
    }

    // Violation: publish inside a while loop.
    pub fn drain_queue(env: Env, mut queue: Vec<Address>) {
        while let Some(to) = queue.pop_front() {
            env.events().publish((symbol_short!("drain"),), to);
        }
    }

    // Safe: single summary event after the loop.
    pub fn notify_all_summary(env: Env, recipients: Vec<Address>) {
        for _to in recipients.iter() {}
        env.events()
            .publish((symbol_short!("paid_all"),), recipients.len());
    }

    // Safe: no loop at all.
    pub fn notify_one(env: Env, to: Address) {
        env.events().publish((symbol_short!("paid"), to), 1_i128);
    }
}
