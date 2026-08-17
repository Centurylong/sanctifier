//! Reusable, parameterizable invariant templates for the three properties
//! almost every contract needs to state: conservation, monotonicity, and
//! access-control.
//!
//! These are companions to [`crate::invariant::scan_invariant_attrs`]: rather
//! than hand-writing a fresh SMT-checkable expression for every contract, a
//! contract author picks a template function from this module and drops its
//! rendered string straight into a `#[sanctify::invariant(...)]` attribute —
//! adoption is a single line, and the expression stays consistent (and
//! greppable) across the whole codebase.
//!
//! ```
//! use sanctifier_core::invariant_library::conservation::total_preserved;
//!
//! assert_eq!(
//!     total_preserved("total_supply"),
//!     "pure::conservation::total_preserved(before.total_supply, after.total_supply)"
//! );
//! ```

/// Conservation invariants: a quantity's total is unchanged across a
/// state transition (e.g. token supply across a transfer, pooled reserves
/// across a swap).
pub mod conservation {
    /// `before.<field>` equals `after.<field>` — nothing was created or
    /// destroyed by the transition.
    ///
    /// # Example
    /// ```rust,ignore
    /// #[sanctify::invariant(sanctifier_core::invariant_library::conservation::total_preserved("total_supply"))]
    /// #[contractimpl]
    /// impl Token { /* ... */ }
    /// ```
    pub fn total_preserved(field: &str) -> String {
        format!("pure::conservation::total_preserved(before.{field}, after.{field})")
    }

    /// The sum of two split parts of a quantity equals the pre-split total
    /// (e.g. `from_balance + to_balance` is unchanged by a transfer).
    pub fn split_sum_preserved(part_a: &str, part_b: &str) -> String {
        format!(
            "pure::conservation::split_sum_preserved(before.{part_a} + before.{part_b}, after.{part_a} + after.{part_b})"
        )
    }
}

/// Monotonicity invariants: a quantity only ever moves in one direction
/// across state transitions (e.g. a nonce, a cumulative fee counter, a
/// ledger-sequence-gated unlock).
pub mod monotonicity {
    /// `after.<field> >= before.<field>` — the field never decreases.
    pub fn non_decreasing(field: &str) -> String {
        format!("pure::monotonicity::non_decreasing(before.{field}, after.{field})")
    }

    /// `after.<field> <= before.<field>` — the field never increases.
    pub fn non_increasing(field: &str) -> String {
        format!("pure::monotonicity::non_increasing(before.{field}, after.{field})")
    }
}

/// Access-control invariants: a state-mutating transition may only be
/// attributed to an authorized caller.
pub mod access_control {
    /// The caller performing this transition must equal the address stored
    /// in `role_field` (e.g. `"admin"`).
    pub fn only_role(role_field: &str) -> String {
        format!("pure::access_control::only_role(caller, before.{role_field})")
    }

    /// The caller must be present in the set stored in `role_set_field`
    /// (e.g. a multi-admin allowlist).
    pub fn caller_in_role_set(role_set_field: &str) -> String {
        format!("pure::access_control::caller_in_role_set(caller, before.{role_set_field})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invariant::scan_invariant_attrs;

    /// Every template must render to a valid `#[sanctify::invariant(...)]`
    /// expression the existing scanner can extract — this is the "one line
    /// to adopt" acceptance bar.
    fn assert_adoptable(expr: &str) {
        let source = format!(
            r#"
            use soroban_sdk::{{contractimpl, Env}};

            #[sanctify::invariant({expr})]
            #[contractimpl]
            impl Contract {{
                pub fn noop(_env: Env) {{}}
            }}
        "#
        );

        let decls = scan_invariant_attrs(&source, "example.rs");
        assert_eq!(
            decls.len(),
            1,
            "template did not scan as an invariant: {expr}"
        );
        // `expr_str` is re-serialized from tokens by `quote!`, which spaces
        // punctuation (`pure :: conservation :: ...`), so compare with
        // whitespace stripped rather than an exact substring.
        assert!(
            decls[0].expr_str.replace(' ', "").contains("pure::"),
            "expected a pure:: call, got: {}",
            decls[0].expr_str
        );
    }

    #[test]
    fn conservation_templates_are_adoptable() {
        assert_adoptable(&conservation::total_preserved("total_supply"));
        assert_adoptable(&conservation::split_sum_preserved(
            "from_balance",
            "to_balance",
        ));
    }

    #[test]
    fn monotonicity_templates_are_adoptable() {
        assert_adoptable(&monotonicity::non_decreasing("nonce"));
        assert_adoptable(&monotonicity::non_increasing("unlock_ledger"));
    }

    #[test]
    fn access_control_templates_are_adoptable() {
        assert_adoptable(&access_control::only_role("admin"));
        assert_adoptable(&access_control::caller_in_role_set("admins"));
    }

    #[test]
    fn total_preserved_renders_expected_expression() {
        assert_eq!(
            conservation::total_preserved("total_supply"),
            "pure::conservation::total_preserved(before.total_supply, after.total_supply)"
        );
    }
}
