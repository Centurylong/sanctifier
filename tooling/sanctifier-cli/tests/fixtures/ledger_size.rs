#![no_std]
use soroban_sdk::{contracttype, Bytes};

#[contracttype]
pub struct LargeStruct {
    pub data1: Bytes, // 64 bytes
    pub data2: Bytes, // 64 bytes
    pub data3: Bytes, // 64 bytes
    pub data4: Bytes, // 64 bytes
}

#[contracttype]
pub enum LargeEnum {
    A(LargeStruct),
    B(LargeStruct),
}
