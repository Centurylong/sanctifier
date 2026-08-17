//! Fuzz the analyzer against malformed/adversarial Rust input (#765).
//!
//! An analyzer that panics or hangs on weird input is unreliable in CI — a
//! single malformed contract file in a scanned repo should degrade to "no
//! findings for this file", not take down the whole run. This runs as a
//! normal `cargo test` (already part of the CI `Run All Tests` step) rather
//! than a `cargo-fuzz`/libFuzzer harness, so it needs no nightly toolchain
//! or extra CI job: every entry point below is exercised with a fixed,
//! deterministically-seeded batch of adversarial inputs and wrapped in
//! `catch_unwind`, so a regression fails the existing test suite directly.
//!
//! Two input strategies, mirroring real-world "hostile input":
//!   * **byte soup** — pseudo-random bytes from a charset that skews toward
//!     tokens the parser cares about (braces, quotes, Soroban keywords), to
//!     more often reach the parts of the AST that detectors walk.
//!   * **mutated fixtures** — real, previously-valid detector fixtures with
//!     random deletions/insertions/substitutions applied, so we fuzz "almost
//!     valid" Rust, not just noise `syn` will always reject outright.

use sanctifier_core::rules::RuleRegistry;
use sanctifier_core::{Analyzer, SanctifyConfig};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

/// Small, dependency-free xorshift64 PRNG so the fuzz corpus is
/// deterministic (reproducible failures) without pulling in `rand` just for
/// tests.
struct Xorshift64(u64);

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_range(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() as usize) % bound
        }
    }
}

const BYTE_SOUP_CHARSET: &[u8] =
    b"{}()[]<>;:,.\"'!?#&*+-/=|^%$@ \n\tfnletmutpubstructimplenumtraitforwhileifelsereturnAddressEnvBytesu32u64i128storageinstancepersistentrequire_authcontracttype0123456789_";

fn random_byte_soup(rng: &mut Xorshift64, max_len: usize) -> String {
    let len = rng.next_range(max_len);
    let mut s = String::with_capacity(len);
    for _ in 0..len {
        let idx = rng.next_range(BYTE_SOUP_CHARSET.len());
        s.push(BYTE_SOUP_CHARSET[idx] as char);
    }
    s
}

const FIXTURES_TO_MUTATE: &[&str] = &[
    include_str!("fixtures/detectors/auth_gap.rs"),
    include_str!("fixtures/detectors/arithmetic_overflow.rs"),
    include_str!("fixtures/detectors/state_write_in_view.rs"),
    include_str!("fixtures/detectors/unbounded_storage.rs"),
];

/// Apply a handful of random char-level mutations (delete/insert/replace) to
/// an otherwise-valid fixture, producing "almost valid" adversarial Rust.
fn mutate_fixture(rng: &mut Xorshift64, source: &str) -> String {
    let mut chars: Vec<char> = source.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mutations = 1 + rng.next_range(20);
    for _ in 0..mutations {
        if chars.is_empty() {
            break;
        }
        let pos = rng.next_range(chars.len());
        match rng.next_range(3) {
            0 => {
                chars.remove(pos);
            }
            1 => {
                let idx = rng.next_range(BYTE_SOUP_CHARSET.len());
                chars.insert(pos, BYTE_SOUP_CHARSET[idx] as char);
            }
            _ => {
                let idx = rng.next_range(BYTE_SOUP_CHARSET.len());
                chars[pos] = BYTE_SOUP_CHARSET[idx] as char;
            }
        }
    }
    chars.into_iter().collect()
}

type FuzzCase<'a> = (&'a str, Box<dyn Fn() + 'a>);

/// Run every core detector entry point against `source`, asserting none of
/// them panic. Each entry point is caught independently so one panicking
/// detector doesn't hide failures in the others.
fn assert_no_panic_on(registry: &RuleRegistry, analyzer: &Analyzer, source: &str, label: &str) {
    let cases: Vec<FuzzCase> = vec![
        (
            "RuleRegistry::run_all",
            Box::new(|| {
                registry.run_all(source);
            }),
        ),
        (
            "Analyzer::scan_storage_collisions",
            Box::new(|| {
                analyzer.scan_storage_collisions(source);
            }),
        ),
        (
            "Analyzer::analyze_ledger_size",
            Box::new(|| {
                analyzer.analyze_ledger_size(source);
            }),
        ),
        (
            "Analyzer::analyze_unsafe_patterns",
            Box::new(|| {
                analyzer.analyze_unsafe_patterns(source);
            }),
        ),
        (
            "Analyzer::scan_auth_gaps",
            Box::new(|| {
                analyzer.scan_auth_gaps(source);
            }),
        ),
        (
            "Analyzer::scan_panics",
            Box::new(|| {
                analyzer.scan_panics(source);
            }),
        ),
        (
            "Analyzer::scan_arithmetic_overflow",
            Box::new(|| {
                analyzer.scan_arithmetic_overflow(source);
            }),
        ),
        (
            "Analyzer::scan_events",
            Box::new(|| {
                analyzer.scan_events(source);
            }),
        ),
        (
            "Analyzer::scan_unhandled_results",
            Box::new(|| {
                analyzer.scan_unhandled_results(source);
            }),
        ),
        (
            "Analyzer::analyze_upgrade_patterns",
            Box::new(|| {
                analyzer.analyze_upgrade_patterns(source);
            }),
        ),
    ];

    for (name, run) in cases {
        let start = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(run));
        let elapsed = start.elapsed();

        assert!(
            result.is_ok(),
            "{name} panicked on {label} input: {source:?}"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "{name} took {elapsed:?} (possible hang) on {label} input: {source:?}"
        );
    }
}

#[test]
fn fuzz_byte_soup_does_not_panic_or_hang() {
    let registry = RuleRegistry::with_default_rules();
    let analyzer = Analyzer::new(SanctifyConfig::default());
    let mut rng = Xorshift64::new(0xC0FFEE_u64);

    for i in 0..150 {
        let input = random_byte_soup(&mut rng, 512);
        assert_no_panic_on(&registry, &analyzer, &input, &format!("byte-soup #{i}"));
    }
}

#[test]
fn fuzz_mutated_fixtures_do_not_panic_or_hang() {
    let registry = RuleRegistry::with_default_rules();
    let analyzer = Analyzer::new(SanctifyConfig::default());
    let mut rng = Xorshift64::new(0xDEADBEEF_u64);

    for (fixture_idx, fixture) in FIXTURES_TO_MUTATE.iter().enumerate() {
        for i in 0..25 {
            let input = mutate_fixture(&mut rng, fixture);
            assert_no_panic_on(
                &registry,
                &analyzer,
                &input,
                &format!("mutated-fixture #{fixture_idx}/{i}"),
            );
        }
    }
}

#[test]
fn fuzz_edge_case_inputs_do_not_panic_or_hang() {
    let registry = RuleRegistry::with_default_rules();
    let analyzer = Analyzer::new(SanctifyConfig::default());

    let edge_cases = [
        "",
        " ",
        "\n\n\n",
        "fn",
        "fn f(",
        "struct",
        "{{{{{{{{{{",
        "}}}}}}}}}}",
        "///",
        "// sanctifier-ignore:",
        "#[contracttype]",
        "\0\0\0",
        "\"unterminated string",
        "'\\u{FFFFFF}'",
        "fn f() { let x: u64 = 999999999999999999999999999999; }",
    ];

    for (i, case) in edge_cases.iter().enumerate() {
        assert_no_panic_on(&registry, &analyzer, case, &format!("edge-case #{i}"));
    }
}
