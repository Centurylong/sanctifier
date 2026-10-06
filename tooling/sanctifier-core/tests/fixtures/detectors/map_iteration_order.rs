#![no_std]
use soroban_sdk::{contract, contractimpl, Address, Map};

#[contract]
pub struct MapOrderContract;

#[contractimpl]
impl MapOrderContract {
    // Order-dependent: selects the first entry from a Map iterator.
    pub fn first_entry(entries: Map<Address, i128>) -> Option<(Address, i128)> {
        entries.iter().next()
    }

    // Order-dependent: selects the last entry from a Map iterator.
    pub fn last_entry(entries: Map<Address, i128>) -> Option<(Address, i128)> {
        entries.iter().last()
    }

    // Order-dependent: index selection over iteration order.
    pub fn second_entry(entries: Map<Address, i128>) -> Option<(Address, i128)> {
        entries.iter().nth(1)
    }

    // Order-dependent: find returns the first matching entry.
    pub fn first_positive(entries: Map<Address, i128>) -> Option<(Address, i128)> {
        entries.iter().find(|(_, value)| *value > 0)
    }

    // Order-dependent: returns a key from the first matching loop iteration.
    pub fn pick_large(entries: Map<Address, i128>) -> Option<Address> {
        for (key, value) in entries.iter() {
            if value > 10 {
                return Some(key);
            }
        }
        None
    }

    // Order-dependent: stores a loop item, then breaks.
    pub fn pick_with_break(entries: Map<Address, i128>, fallback: Address) -> Address {
        let mut winner = fallback;
        for (key, value) in entries.iter() {
            if value > 10 {
                winner = key;
                break;
            }
        }
        winner
    }

    // Safe for this advisory: visits every entry and uses commutative addition.
    pub fn sum_all(entries: Map<Address, i128>) -> i128 {
        let mut total = 0_i128;
        for (_, value) in entries.iter() {
            total += value;
        }
        total
    }

    // Safe for this advisory: direct keyed lookup does not depend on iteration.
    pub fn keyed_lookup(entries: Map<Address, i128>, key: Address) -> Option<i128> {
        entries.get(key)
    }

    // Safe for this advisory: all/any outcomes do not select an entry by order.
    pub fn all_non_negative(entries: Map<Address, i128>) -> bool {
        entries.iter().all(|(_, value)| value >= 0)
    }
}
