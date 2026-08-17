//! Finding priority/ranking model.
//!
//! [`RuleViolation::severity`](crate::rules::Severity) is a coarse, 3-level
//! signal (`Error` / `Warning` / `Info`). That is enough to bucket findings,
//! but it can't tell a reentrancy-shaped auth bypass apart from a minor
//! style nit that both happen to be `Error`. This module adds a small,
//! numeric [`priority_score`] on top of severity so findings can be sorted
//! into a single, meaningful "most urgent first" ordering.
//!
//! This is deliberately a simple, transparent scoring model — a lookup
//! table plus a fixed boost for a short, hardcoded list of especially
//! security-critical rules — **not** a learned/ML model. All constants live
//! in this file so the ranking is easy to read, reason about, and retune.
//!
//! Nothing here changes `RuleViolation` or how rules run; it's a pure,
//! additive read of already-produced violations.

use crate::rules::{RuleViolation, Severity};

/// Base score contributed by [`Severity::Error`].
///
/// Errors are the most actionable class of finding (likely bugs / exploit
/// paths), so they anchor the top of the score range.
pub const SEVERITY_SCORE_ERROR: u8 = 70;

/// Base score contributed by [`Severity::Warning`].
pub const SEVERITY_SCORE_WARNING: u8 = 40;

/// Base score contributed by [`Severity::Info`].
///
/// Informational findings are still worth surfacing, but should always sort
/// below every `Warning` and `Error`.
pub const SEVERITY_SCORE_INFO: u8 = 10;

/// Extra weight added on top of the severity base score when a violation's
/// `rule_name` is one of [`CRITICAL_RULE_NAMES`].
///
/// Chosen so that a critical `Error` (70 + 20 = 90) still ranks below the
/// theoretical maximum (100), leaving headroom for future finer-grained
/// weighting, while comfortably outranking every non-critical `Error` (70)
/// and — importantly — never lets a critical `Info`/`Warning` finding
/// outrank a non-critical `Error`... except by design a critical `Warning`
/// (40 + 20 = 60) still sits below a plain `Error` (70). Severity remains
/// the dominant signal; the boost only breaks ties *within* a severity
/// band.
pub const CRITICAL_RULE_BOOST: u8 = 20;

/// Rule names (as returned by `Rule::name()`) treated as especially
/// security-critical: exploitable classes such as missing authorization,
/// admin takeover, and approve/allowance front-running races. Findings from
/// these rules are boosted above other findings of the same severity so
/// they surface first during triage.
///
/// Sourced directly from each rule's `fn name(&self) -> &str` in
/// `tooling/sanctifier-core/src/rules/`:
/// - `auth_gap` (`auth_gap.rs`, `AuthGapRule`) — public state mutation with no
///   authorization check (auth bypass).
/// - `sanct_visibility` (`auth_gap.rs`, `VisibilityLeakRule`) — public
///   helper-shaped methods that mutate state without authorization.
/// - `wrong_auth_args` (`wrong_auth_args.rs`) — `require_auth()` that fails
///   to bind the specific arguments being authorized (auth bypass variant).
/// - `init_hardcoded_admin` (`init_hardcoded_admin.rs`) — hardcoded admin
///   address/literal baked into initialization (admin takeover).
/// - `allowance_race` (`allowance_race.rs`) — unconditional allowance
///   overwrites lacking compare-and-set semantics (approve front-run race,
///   the reentrancy-adjacent class of finding).
pub const CRITICAL_RULE_NAMES: &[&str] = &[
    "auth_gap",
    "sanct_visibility",
    "wrong_auth_args",
    "init_hardcoded_admin",
    "allowance_race",
];

/// Maps a [`Severity`] to its base priority score.
fn severity_base_score(severity: Severity) -> u8 {
    match severity {
        Severity::Error => SEVERITY_SCORE_ERROR,
        Severity::Warning => SEVERITY_SCORE_WARNING,
        Severity::Info => SEVERITY_SCORE_INFO,
    }
}

/// Returns `true` if `rule_name` is on the hardcoded critical-rules list.
fn is_critical_rule(rule_name: &str) -> bool {
    CRITICAL_RULE_NAMES.contains(&rule_name)
}

/// Computes a 0-100 priority score for `violation`, where higher means more
/// urgent.
///
/// Score = `severity_base_score(violation.severity)` + `CRITICAL_RULE_BOOST`
/// if `violation.rule_name` is in [`CRITICAL_RULE_NAMES`], else + 0. The sum
/// is clamped to 100 (a defensive bound; with the current constants the
/// maximum attainable value is 90, so the clamp is a no-op today, but it
/// keeps the documented 0-100 contract true if constants are retuned).
///
/// This is a simple, transparent, deterministic model: same inputs always
/// produce the same score, and the score is fully explained by the two
/// constants above. It is not a machine-learned ranking.
pub fn priority_score(violation: &RuleViolation) -> u8 {
    let base = severity_base_score(violation.severity);
    let boost = if is_critical_rule(&violation.rule_name) {
        CRITICAL_RULE_BOOST
    } else {
        0
    };
    base.saturating_add(boost).min(100)
}

/// Sorts `violations` in place, most urgent first.
///
/// Primary key is [`priority_score`] (descending). Ties are broken
/// deterministically by `location` then `rule_name` (both ascending), so
/// repeated runs over the same input always produce the same order
/// regardless of the order rules happened to run in.
pub fn sort_by_priority(violations: &mut [RuleViolation]) {
    violations.sort_by(|a, b| {
        priority_score(b)
            .cmp(&priority_score(a))
            .then_with(|| a.location.cmp(&b.location))
            .then_with(|| a.rule_name.cmp(&b.rule_name))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn violation(rule_name: &str, severity: Severity, location: &str) -> RuleViolation {
        RuleViolation::new(
            rule_name,
            severity,
            format!("{rule_name} message"),
            location.to_string(),
        )
    }

    #[test]
    fn severity_alone_orders_error_above_warning_above_info() {
        let error = violation("some_rule", Severity::Error, "a:1");
        let warning = violation("some_rule", Severity::Warning, "a:1");
        let info = violation("some_rule", Severity::Info, "a:1");

        let error_score = priority_score(&error);
        let warning_score = priority_score(&warning);
        let info_score = priority_score(&info);

        assert!(error_score > warning_score);
        assert!(warning_score > info_score);
        assert_eq!(error_score, SEVERITY_SCORE_ERROR);
        assert_eq!(warning_score, SEVERITY_SCORE_WARNING);
        assert_eq!(info_score, SEVERITY_SCORE_INFO);
    }

    #[test]
    fn critical_rule_outranks_non_critical_rule_at_same_severity() {
        assert!(is_critical_rule("auth_gap"));
        assert!(!is_critical_rule("unused_variable"));

        let critical = violation("auth_gap", Severity::Error, "lib.rs:10");
        let non_critical = violation("unused_variable", Severity::Error, "lib.rs:10");

        assert!(priority_score(&critical) > priority_score(&non_critical));
        assert_eq!(
            priority_score(&critical),
            SEVERITY_SCORE_ERROR + CRITICAL_RULE_BOOST
        );
    }

    #[test]
    fn all_critical_rule_names_are_recognized() {
        for name in CRITICAL_RULE_NAMES {
            assert!(is_critical_rule(name));
        }
    }

    #[test]
    fn score_is_clamped_to_one_hundred() {
        // Defensive check: even if constants were retuned to overflow, the
        // documented 0-100 contract must hold.
        let v = violation("auth_gap", Severity::Error, "a:1");
        assert!(priority_score(&v) <= 100);
    }

    #[test]
    fn sort_by_priority_orders_descending_and_breaks_ties_deterministically() {
        let mut violations = vec![
            violation("unused_variable", Severity::Info, "z:9"),
            violation("auth_gap", Severity::Error, "b:2"),
            violation("missing_ttl", Severity::Warning, "c:3"),
            violation("allowance_race", Severity::Error, "a:1"),
            // Same score as the previous critical Error but a "smaller"
            // location, so the tie-break should place it first among equals.
            violation("init_hardcoded_admin", Severity::Error, "a:0"),
        ];

        sort_by_priority(&mut violations);

        let scores: Vec<u8> = violations.iter().map(priority_score).collect();
        // Descending order overall.
        for pair in scores.windows(2) {
            assert!(pair[0] >= pair[1]);
        }

        // The two critical Errors (both scoring 90) come first, ordered by
        // location ("a:0" < "a:1"), ahead of the non-critical Error (70).
        assert_eq!(violations[0].rule_name, "init_hardcoded_admin");
        assert_eq!(violations[0].location, "a:0");
        assert_eq!(violations[1].rule_name, "allowance_race");
        assert_eq!(violations[1].location, "a:1");
        assert_eq!(violations[2].rule_name, "auth_gap");

        // Warning still outranks Info.
        let warning_pos = violations
            .iter()
            .position(|v| v.rule_name == "missing_ttl")
            .unwrap();
        let info_pos = violations
            .iter()
            .position(|v| v.rule_name == "unused_variable")
            .unwrap();
        assert!(warning_pos < info_pos);
    }

    #[test]
    fn sort_by_priority_is_deterministic_across_runs() {
        let build = || {
            vec![
                violation("wrong_auth_args", Severity::Error, "x:1"),
                violation("sanct_visibility", Severity::Error, "x:1"),
                violation("panic_detection", Severity::Warning, "y:1"),
            ]
        };

        let mut first = build();
        let mut second = build();
        sort_by_priority(&mut first);
        sort_by_priority(&mut second);

        let first_names: Vec<&str> = first.iter().map(|v| v.rule_name.as_str()).collect();
        let second_names: Vec<&str> = second.iter().map(|v| v.rule_name.as_str()).collect();
        assert_eq!(first_names, second_names);
    }
}
