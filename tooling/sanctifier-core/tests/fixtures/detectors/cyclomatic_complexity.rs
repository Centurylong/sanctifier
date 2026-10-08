#![no_std]
use soroban_sdk::{contract, contractimpl};

#[contract]
pub struct Hotspot;

#[contractimpl]
impl Hotspot {
    pub fn hotspot(value: i32) -> i32 {
        let mut score = 0;
        if value > 0 { score += 1; }
        if value > 1 { score += 1; }
        if value > 2 { score += 1; }
        if value > 3 { score += 1; }
        if value > 4 { score += 1; }
        if value > 5 { score += 1; }
        if value > 6 { score += 1; }
        if value > 7 { score += 1; }
        if value > 8 { score += 1; }
        if value > 9 { score += 1; }
        if value > 10 { score += 1; }
        score
    }

    pub fn simple(value: i32) -> i32 {
        if value > 0 { 1 } else { 0 }
    }
}
