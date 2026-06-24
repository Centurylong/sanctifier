use criterion::{criterion_group, criterion_main, Criterion};

use ark_std::rand::SeedableRng;
use sanctifier_zk::{encoding, prove, setup, verify, N_RULES};

fn bench_setup(c: &mut Criterion) {
    c.bench_function("groth16_setup", |b| {
        b.iter(|| {
            let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
            setup(&mut rng).expect("setup failed")
        });
    });
}

fn bench_prove(c: &mut Criterion) {
    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
    let (pk, _vk) = setup(&mut rng).expect("setup failed");

    let rule_results: [bool; N_RULES] = std::array::from_fn(|i| i < 9); // 9/11 pass
    let wasm_bytes = b"\x00asm\x01\x00\x00\x00".as_ref();

    let stmt = sanctifier_zk::AuditStatement {
        wasm_hash: encoding::wasm_hash_field(wasm_bytes),
        ruleset_version: encoding::ruleset_version_field(1),
        score_threshold: encoding::score_threshold_field(9),
        rules_commitment: encoding::rules_commitment(&rule_results),
    };

    c.bench_function("groth16_prove", |b| {
        b.iter(|| {
            let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
            prove(&stmt, &rule_results, &pk, &mut rng).expect("prove failed")
        });
    });
}

fn bench_verify(c: &mut Criterion) {
    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
    let (pk, vk) = setup(&mut rng).expect("setup failed");

    let rule_results: [bool; N_RULES] = std::array::from_fn(|i| i < 9);
    let wasm_bytes = b"\x00asm\x01\x00\x00\x00".as_ref();

    let stmt = sanctifier_zk::AuditStatement {
        wasm_hash: encoding::wasm_hash_field(wasm_bytes),
        ruleset_version: encoding::ruleset_version_field(1),
        score_threshold: encoding::score_threshold_field(9),
        rules_commitment: encoding::rules_commitment(&rule_results),
    };

    let mut rng2 = ark_std::rand::rngs::StdRng::seed_from_u64(99);
    let proof = prove(&stmt, &rule_results, &pk, &mut rng2).expect("prove failed");

    c.bench_function("groth16_verify", |b| {
        b.iter(|| verify(&stmt, &proof, &vk).expect("verify failed"));
    });
}

fn bench_proof_size(c: &mut Criterion) {
    let mut rng = ark_std::rand::rngs::StdRng::seed_from_u64(42);
    let (pk, _vk) = setup(&mut rng).expect("setup failed");

    let rule_results: [bool; N_RULES] = std::array::from_fn(|i| i < 9);
    let wasm_bytes = b"\x00asm\x01\x00\x00\x00".as_ref();

    let stmt = sanctifier_zk::AuditStatement {
        wasm_hash: encoding::wasm_hash_field(wasm_bytes),
        ruleset_version: encoding::ruleset_version_field(1),
        score_threshold: encoding::score_threshold_field(9),
        rules_commitment: encoding::rules_commitment(&rule_results),
    };

    let mut rng2 = ark_std::rand::rngs::StdRng::seed_from_u64(99);
    let proof = prove(&stmt, &rule_results, &pk, &mut rng2).expect("prove failed");
    let bytes = sanctifier_zk::proof_to_bytes(&proof);

    // Report proof size outside benchmarking loop — not a throughput benchmark
    c.bench_function("proof_size_bytes (constant)", |b| {
        b.iter(|| {
            // Just measure the serialisation overhead
            sanctifier_zk::proof_to_bytes(&proof).len()
        });
    });

    // Print once to stderr so it shows in `cargo bench` output
    eprintln!("\n[bench] Groth16/BLS12-381 compressed proof size: {} bytes\n", bytes.len());

    // Report constraint count
    let n = sanctifier_zk::constraint_count();
    eprintln!("[bench] R1CS constraint count: {n}\n");
}

criterion_group!(benches, bench_setup, bench_prove, bench_verify, bench_proof_size);
criterion_main!(benches);
