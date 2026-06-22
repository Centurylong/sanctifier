#![no_std]
use soroban_sdk::contracterror;

// FIXTURE: error_code_collision detector
// One enum reuses a discriminant; another mixes explicit and implicit styles.

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorWithDuplicates {
    NotFound = 1,
    Invalid = 1, // Duplicate discriminant!
    Unauthorized = 2,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ErrorInconsistent {
    NotFound = 1,
    Invalid,      // Implicit discriminant mixed with explicit ones
    Unauthorized = 3,
}
