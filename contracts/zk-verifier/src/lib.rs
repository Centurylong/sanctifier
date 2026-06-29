#![no_std]
extern crate alloc;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use ark_bls12_381::{Bls12_381, Fr};
use ark_groth16::{Groth16, Proof, VerifyingKey};
use ark_snark::SNARK;
use ark_serialize::CanonicalDeserialize;
use soroban_sdk::{contract, contractimpl, contracttype, Bytes, Env};

#[contract]
pub struct ZkVerifierContract;

#[contracttype]
pub enum DataKey {
    VerifyingKey,
}

#[contractimpl]
impl ZkVerifierContract {
    pub fn init(env: Env, vk_bytes: Bytes) {
        if env.storage().instance().has(&DataKey::VerifyingKey) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::VerifyingKey, &vk_bytes);
    }

    pub fn verify(
        env: Env,
        proof_bytes: Bytes,
        public_inputs_bytes: Bytes,
    ) -> bool {
        let vk_bytes: Bytes = env
            .storage()
            .instance()
            .get(&DataKey::VerifyingKey)
            .expect("Not initialized");

        let mut vk_slice = [0u8; 1024];
        let vk_len = vk_bytes.len() as usize;
        if vk_len > vk_slice.len() { return false; }
        vk_bytes.copy_into_slice(&mut vk_slice[..vk_len]);

        let mut proof_slice = [0u8; 512];
        let proof_len = proof_bytes.len() as usize;
        if proof_len > proof_slice.len() { return false; }
        proof_bytes.copy_into_slice(&mut proof_slice[..proof_len]);

        let vk = match VerifyingKey::<Bls12_381>::deserialize_compressed(&vk_slice[..vk_len]) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let proof = match Proof::<Bls12_381>::deserialize_compressed(&proof_slice[..proof_len]) {
            Ok(p) => p,
            Err(_) => return false,
        };

        // We know we have 4 public inputs (Fr) in sanctifier-zk
        // Each Fr compressed is 32 bytes
        if public_inputs_bytes.len() != 4 * 32 {
            return false;
        }

        let mut inputs_slice = [0u8; 128];
        public_inputs_bytes.copy_into_slice(&mut inputs_slice);

        // Deserializing Fr uses default as a placeholder
        let mut inputs = [Fr::from(0u8); 4];
        for i in 0..4 {
            let start = i * 32;
            let end = start + 32;
            inputs[i] = match Fr::deserialize_compressed(&inputs_slice[start..end]) {
                Ok(f) => f,
                Err(_) => return false,
            };
        }

        let pvk = Groth16::<Bls12_381>::process_vk(&vk).unwrap();
        Groth16::<Bls12_381>::verify_with_processed_vk(&pvk, &inputs, &proof).unwrap_or(false)
    }
}
