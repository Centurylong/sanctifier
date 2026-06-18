use sanctifier_core::invariant::{scan_invariant_attrs, InvariantVerifyResult};

/// Verify the scanner correctly extracts invariants from a realistic source
/// snippet that mirrors how token-invariants/src/lib.rs is written.
#[test]
fn integration_scan_finds_invariant_in_realistic_source() {
    let source = r#"
        use sanctify_macros::invariant;
        use soroban_sdk::{contract, contractimpl, Env};

        #[contract]
        pub struct Token;

        #[invariant(pure::supply_is_conserved_after_transfer(0, 0, 0))]
        #[contractimpl]
        impl Token {
            pub fn total_supply(_env: Env) -> i128 { 0 }
            pub fn transfer(_env: Env) {}
        }
    "#;

    let decls = scan_invariant_attrs(source, "contracts/token-invariants/src/lib.rs");
    assert_eq!(decls.len(), 1, "expected exactly one invariant declaration");

    let d = &decls[0];
    assert!(
        d.expr_str.contains("supply_is_conserved"),
        "expr_str should contain the invariant function name, got: {}",
        d.expr_str
    );
    assert!(
        d.location.contains("token-invariants"),
        "location should include the file path, got: {}",
        d.location
    );
}

/// Verify that a source file with no #[sanctify::invariant] or #[invariant]
/// attribute returns an empty vec.
#[test]
fn integration_scan_empty_on_unannotated_contract() {
    let source = r#"
        use soroban_sdk::{contract, contractimpl, Env};

        #[contract]
        pub struct Counter;

        #[contractimpl]
        impl Counter {
            pub fn increment(_env: Env) -> u32 { 0 }
        }
    "#;
    let decls = scan_invariant_attrs(source, "counter.rs");
    assert!(decls.is_empty());
}

/// Verify the scanner handles the fully-qualified `sanctify::invariant` form.
#[test]
fn integration_scan_qualified_attribute() {
    let source = r#"
        #[sanctify::invariant(a == b)]
        impl SomeContract {
            pub fn method() {}
        }
    "#;
    let decls = scan_invariant_attrs(source, "qualified.rs");
    assert_eq!(decls.len(), 1);
    assert!(decls[0].expr_str.contains("=="));
}

/// Verify SMT fast-path: integer equality tautology is Proven.
#[cfg(feature = "smt")]
#[test]
fn integration_smt_proves_integer_tautology() {
    use sanctifier_core::invariant::{InvariantDecl, SmtInvariantVerifier};

    let decl = InvariantDecl {
        contract_name: "Token".into(),
        expr_str: "100 == 100".into(),
        location: "test:1".into(),
    };
    let result = SmtInvariantVerifier::new().verify_one(&decl);
    assert_eq!(result, InvariantVerifyResult::Proven);
}

/// Verify SMT fast-path: function-call expression returns Unsupported.
#[cfg(feature = "smt")]
#[test]
fn integration_smt_unsupported_for_function_invariant() {
    use sanctifier_core::invariant::{InvariantDecl, SmtInvariantVerifier};

    let decl = InvariantDecl {
        contract_name: "Token".into(),
        expr_str: "total_supply() == sum_of_balances()".into(),
        location: "test:1".into(),
    };
    let result = SmtInvariantVerifier::new().verify_one(&decl);
    assert_eq!(result, InvariantVerifyResult::Unsupported);
}
