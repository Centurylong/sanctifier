#![no_std]
use soroban_sdk::{contract, contractimpl, Vec};

// FIXTURE: loop_accumulator detector
// Six unchecked accumulation sites, with fresh bindings and safe APIs as controls.

#[contract]
pub struct LoopAccumulatorContract;

#[contractimpl]
impl LoopAccumulatorContract {
    // Violation: the outer total survives each iteration and an inner shadow.
    pub fn sum_for(values: Vec<i128>) -> i128 {
        let mut total: i128 = 0;
        for value in values.iter() {
            {
                let mut total: i128 = 0;
                total += value; // Fresh shadow: not a loop-carried accumulator.
            }
            total += value;
        }
        total
    }

    // Violation: self-addition in a while loop, including parenthesized targets.
    pub fn sum_while(mut values: Vec<i128>) -> i128 {
        let mut total: i128 = 0;
        while let Some(value) = values.pop_front() {
            total = (total) + value;
        }
        total
    }

    // Violation: the accumulator is the right operand of addition.
    pub fn sum_loop(mut values: Vec<i128>) -> i128 {
        let mut total: i128 = 0;
        loop {
            let Some(value) = values.pop_front() else {
                break;
            };
            total = value + total;
        }
        total
    }

    // Violation: a fresh outer-loop subtotal persists across the inner loop.
    pub fn sum_groups(groups: Vec<Vec<i128>>) -> i128 {
        let mut total: i128 = 0;
        for values in groups.iter() {
            let mut subtotal: i128 = 0;
            for value in values.iter() {
                subtotal += value;
            }
            total = total.saturating_add(subtotal);
        }
        total
    }

    // Violation: an additive chain should produce one finding for the assignment.
    pub fn sum_with_fee(values: Vec<i128>, fee: i128) -> i128 {
        let mut total: i128 = 0;
        for value in values.iter() {
            total = (total + value) + fee;
        }
        total
    }

    // Violation: the while condition is evaluated on every iteration.
    pub fn sum_in_condition(mut total: i128, amount: i128, limit: i128) -> i128 {
        while {
            total += amount;
            total < limit
        } {}
        total
    }

    // Safe: the accumulator additions explicitly handle overflow.
    pub fn safe_apis(values: Vec<i128>) -> Result<i128, ()> {
        let mut checked: i128 = 0;
        let mut saturated: i128 = 0;
        for value in values.iter() {
            checked = checked.checked_add(value).ok_or(())?;
            saturated = saturated.saturating_add(value);
        }
        checked.checked_add(saturated).ok_or(())
    }

    // Not accumulation: each temporary is fresh, and last never adds itself.
    pub fn ordinary_addition(values: Vec<i128>) -> i128 {
        let mut last: i128 = 0;
        for value in values.iter() {
            let mut temporary: i128 = 0;
            temporary += value;
            last = value + 1;
        }
        last
    }

    // Not accumulation: both loop patterns introduce fresh bindings.
    pub fn loop_bindings(mut values: Vec<i128>) {
        for mut value in values.iter() {
            value += 1;
        }
        while let Some(mut value) = values.pop_front() {
            value += 1;
        }
    }

    // No loop is inherited by a deferred closure or async body.
    pub fn deferred_bodies(values: Vec<i128>) {
        let mut closure_total: i128 = 0;
        let mut async_total: i128 = 0;
        for value in values.iter() {
            let _closure = || {
                closure_total += value;
            };
            let _future = async {
                async_total += value;
            };
        }
    }

    // Not loop accumulation: these additions execute before the loop begins.
    pub fn before_loop(mut total: i128, values: Vec<i128>) -> i128 {
        total += 1;
        for _value in {
            total += 1;
            values.iter()
        } {}
        total
    }
}
