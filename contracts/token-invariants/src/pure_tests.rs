//! Unit tests for pure.rs — the Env-free token arithmetic.

#[cfg(test)]
mod tests {
    use crate::pure::*;

    // ── transfer_pure ─────────────────────────────────────────────────────────

    #[test]
    fn transfer_moves_tokens_correctly() {
        let (from, to) = transfer_pure(1_000, 0, 300).unwrap();
        assert_eq!(from, 700);
        assert_eq!(to, 300);
    }

    #[test]
    fn transfer_rejects_zero_amount() {
        assert!(transfer_pure(1_000, 0, 0).is_err());
    }

    #[test]
    fn transfer_rejects_negative_amount() {
        assert!(transfer_pure(1_000, 0, -1).is_err());
    }

    #[test]
    fn transfer_rejects_insufficient_balance() {
        assert!(transfer_pure(100, 0, 101).is_err());
    }

    // ── mint_pure ─────────────────────────────────────────────────────────────

    #[test]
    fn mint_increases_balance() {
        assert_eq!(mint_pure(0, 500).unwrap(), 500);
    }

    #[test]
    fn mint_rejects_non_positive() {
        assert!(mint_pure(0, 0).is_err());
        assert!(mint_pure(0, -5).is_err());
    }

    #[test]
    fn mint_rejects_overflow() {
        assert!(mint_pure(i128::MAX, 1).is_err());
    }

    // ── burn_pure ─────────────────────────────────────────────────────────────

    #[test]
    fn burn_decreases_balance() {
        assert_eq!(burn_pure(500, 200).unwrap(), 300);
    }

    #[test]
    fn burn_rejects_non_positive() {
        assert!(burn_pure(500, 0).is_err());
    }

    #[test]
    fn burn_rejects_insufficient() {
        assert!(burn_pure(50, 51).is_err());
    }

    // ── supply_is_conserved_after_transfer ────────────────────────────────────

    #[test]
    fn supply_conserved_valid_transfer() {
        assert!(supply_is_conserved_after_transfer(1_000, 0, 400));
    }

    #[test]
    fn supply_conserved_trivially_on_invalid_transfer() {
        // amount = 0 is rejected by transfer_pure, so conservation returns true
        assert!(supply_is_conserved_after_transfer(100, 100, 0));
    }
}
