//! Auto-triage: cluster and dedupe near-duplicate rule findings.
//!
//! Scans can produce a pile of `RuleViolation`s that are, for a human
//! reviewer, effectively the same finding repeated: the same detector firing
//! on every iteration of a near-identical code pattern across many similar
//! functions, or two different rules independently flagging the same
//! underlying issue at the same (or adjacent) location with near-identical
//! wording.
//!
//! This module provides a small, pure, additive similarity heuristic to
//! group those findings so a caller can either collapse each group down to
//! one representative ([`dedupe_similar`]) or inspect the groups directly to
//! show something like "12 similar findings, showing 1" ([`cluster_similar`]).
//!
//! # This is intentionally a simple heuristic
//!
//! Two violations are considered "similar" only if they share the same
//! `rule_name`, AND either:
//!
//! - (a) their `location` strings share the same prefix before the final
//!   `:line` component, and the parsed line numbers are within
//!   [`LOCATION_LINE_WINDOW`] lines of each other, OR
//! - (b) their `message` strings are exactly identical.
//!
//! This is a syntactic, line-window + exact-message-match heuristic — **not**
//! semantic or embedding-based clustering. It will not catch findings that
//! are conceptually the same but worded differently, nor findings from the
//! same underlying issue that are far apart in the file. It is deliberately
//! conservative (cheap, deterministic, no false-collapsing of unrelated
//! findings) rather than maximally aggressive at reducing noise.
//!
//! `location` is expected to be in the `"<prefix>:<line>"` format used
//! throughout `rules/*.rs` (e.g. `"transfer:42"`, the enclosing function name
//! and line number). If a `location` string doesn't parse into that shape,
//! it is simply treated as "not similar by location" (falls back to the
//! message-equality check) rather than panicking.

use crate::rules::RuleViolation;

/// Maximum distance (in source lines) between two violations' parsed line
/// numbers for them to be considered part of the same location-based
/// cluster. Tune this constant if the heuristic proves too aggressive or too
/// lax in practice.
pub const LOCATION_LINE_WINDOW: usize = 3;

/// Parses a `"<prefix>:<line>"` location string into `(prefix, line)`.
///
/// Splits on the *last* `:` so that prefixes which themselves might contain
/// `:` (unlikely today, but not guaranteed) are handled reasonably. Returns
/// `None` — rather than panicking — when the string has no `:`, or the
/// suffix after the last `:` isn't a valid line number.
fn parse_location(location: &str) -> Option<(&str, usize)> {
    let (prefix, line_str) = location.rsplit_once(':')?;
    let line = line_str.parse::<usize>().ok()?;
    Some((prefix, line))
}

/// Returns `true` if `a` and `b` are "similar" per this module's heuristic:
/// same `rule_name`, and either a close-by same-prefix location or an
/// identical message.
fn are_similar(a: &RuleViolation, b: &RuleViolation) -> bool {
    if a.rule_name != b.rule_name {
        return false;
    }

    if a.message == b.message {
        return true;
    }

    match (parse_location(&a.location), parse_location(&b.location)) {
        (Some((prefix_a, line_a)), Some((prefix_b, line_b))) => {
            prefix_a == prefix_b && line_a.abs_diff(line_b) <= LOCATION_LINE_WINDOW
        }
        _ => false,
    }
}

/// Groups `violations` into clusters of mutually-similar findings, returning
/// the clusters as borrowed references in original relative order (both the
/// order of clusters, and the order of items within each cluster, match
/// their first appearance in `violations`).
///
/// A violation joins the first existing cluster it is [`are_similar`] to any
/// member of; otherwise it starts a new cluster. This is a simple greedy
/// grouping (not a full transitive-closure clustering pass), which keeps the
/// heuristic cheap and its behavior easy to reason about.
///
/// Useful for callers that want to show something like "12 similar findings,
/// showing 1" instead of silently dropping the rest — see [`dedupe_similar`]
/// for the collapsing variant.
///
/// Empty input returns empty output.
pub fn cluster_similar(violations: &[RuleViolation]) -> Vec<Vec<&RuleViolation>> {
    let mut clusters: Vec<Vec<&RuleViolation>> = Vec::new();

    for violation in violations {
        let existing_cluster = clusters
            .iter_mut()
            .find(|cluster| cluster.iter().any(|member| are_similar(member, violation)));

        match existing_cluster {
            Some(cluster) => cluster.push(violation),
            None => clusters.push(vec![violation]),
        }
    }

    clusters
}

/// Clusters `violations` using the same similarity heuristic as
/// [`cluster_similar`], then collapses each cluster down to exactly one
/// representative — the first violation encountered in that cluster, for
/// deterministic output — dropping the rest.
///
/// The returned `Vec` preserves the original relative order of the kept
/// representatives (i.e. it is a stable filter over `violations`, not a
/// reordering by cluster).
///
/// Empty input returns empty output.
pub fn dedupe_similar(violations: Vec<RuleViolation>) -> Vec<RuleViolation> {
    let mut representatives: Vec<RuleViolation> = Vec::new();

    for violation in violations {
        let is_duplicate = representatives
            .iter()
            .any(|kept| are_similar(kept, &violation));
        if !is_duplicate {
            representatives.push(violation);
        }
    }

    representatives
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Severity;

    fn violation(rule_name: &str, message: &str, location: &str) -> RuleViolation {
        RuleViolation::new(
            rule_name,
            Severity::Warning,
            message.to_string(),
            location.to_string(),
        )
    }

    #[test]
    fn dedupe_empty_input_returns_empty() {
        let result = dedupe_similar(Vec::new());
        assert!(result.is_empty());
    }

    #[test]
    fn cluster_empty_input_returns_empty() {
        let result = cluster_similar(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn dedupe_identical_rule_and_message_collapses_to_one() {
        let violations = vec![
            violation("sanct_unwrap", "Unhandled unwrap() call", "entry:10"),
            violation(
                "sanct_unwrap",
                "Unhandled unwrap() call",
                "far_away_fn:9000",
            ),
        ];
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].location, "entry:10");
    }

    #[test]
    fn dedupe_same_rule_lines_within_window_collapses_to_one() {
        let violations = vec![
            violation("arithmetic_overflow", "Unchecked addition", "transfer:10"),
            violation(
                "arithmetic_overflow",
                "Unchecked addition may overflow",
                "transfer:12",
            ),
        ];
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].location, "transfer:10");
    }

    #[test]
    fn dedupe_same_rule_lines_far_apart_stays_separate() {
        let violations = vec![
            violation("arithmetic_overflow", "Unchecked addition", "transfer:10"),
            violation(
                "arithmetic_overflow",
                "Unchecked addition may overflow",
                "transfer:200",
            ),
        ];
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn dedupe_different_rule_name_identical_message_stays_separate() {
        let violations = vec![
            violation("rule_a", "Same message text", "fn_a:10"),
            violation("rule_b", "Same message text", "fn_b:10"),
        ];
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn dedupe_preserves_original_relative_order_of_representatives() {
        let violations = vec![
            violation("rule_a", "msg a", "fn_a:1"),
            violation("rule_b", "msg b", "fn_b:1"),
            violation("rule_a", "msg a", "fn_a:2"), // dupe of first
            violation("rule_c", "msg c", "fn_c:1"),
        ];
        let result = dedupe_similar(violations);
        let rule_names: Vec<&str> = result.iter().map(|v| v.rule_name.as_str()).collect();
        assert_eq!(rule_names, vec!["rule_a", "rule_b", "rule_c"]);
    }

    #[test]
    fn dedupe_unparseable_location_falls_back_to_message_equality_only() {
        let violations = vec![
            violation("rule_a", "distinct message one", "no-colon-here"),
            violation("rule_a", "distinct message two", "also-no-colon"),
        ];
        // Locations don't parse (no ':'), messages differ -> not similar, both kept.
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn dedupe_unparseable_location_but_identical_message_still_collapses() {
        let violations = vec![
            violation("rule_a", "identical message", "no-colon-here"),
            violation("rule_a", "identical message", "also-no-colon"),
        ];
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn cluster_similar_returns_correctly_sized_groups() {
        let violations = vec![
            violation("rule_a", "msg a", "fn_a:1"),  // cluster 0
            violation("rule_b", "msg b", "fn_b:1"),  // cluster 1
            violation("rule_a", "msg a2", "fn_a:2"), // joins cluster 0 (line window)
            violation("rule_c", "msg c", "fn_c:1"),  // cluster 2
            violation("rule_a", "msg a3", "fn_a:3"), // joins cluster 0 (line window)
        ];
        let clusters = cluster_similar(&violations);
        assert_eq!(clusters.len(), 3);

        let sizes: Vec<usize> = clusters.iter().map(|c| c.len()).collect();
        assert_eq!(sizes, vec![3, 1, 1]);

        // The 3-member cluster should be the three rule_a violations, in order.
        assert!(clusters[0].iter().all(|v| v.rule_name == "rule_a"));
        assert_eq!(clusters[0][0].location, "fn_a:1");
        assert_eq!(clusters[0][1].location, "fn_a:2");
        assert_eq!(clusters[0][2].location, "fn_a:3");
    }

    #[test]
    fn cluster_similar_line_window_boundary_is_inclusive() {
        // Exactly LOCATION_LINE_WINDOW apart -> same cluster.
        let violations = vec![
            violation("rule_a", "msg a", "fn_a:10"),
            violation(
                "rule_a",
                "msg a variant",
                &format!("fn_a:{}", 10 + LOCATION_LINE_WINDOW),
            ),
        ];
        let clusters = cluster_similar(&violations);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].len(), 2);
    }

    #[test]
    fn cluster_similar_line_window_boundary_plus_one_is_separate() {
        let violations = vec![
            violation("rule_a", "msg a", "fn_a:10"),
            violation(
                "rule_a",
                "msg a variant",
                &format!("fn_a:{}", 10 + LOCATION_LINE_WINDOW + 1),
            ),
        ];
        let clusters = cluster_similar(&violations);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn dedupe_does_not_panic_on_mixed_valid_and_invalid_locations() {
        let violations = vec![
            violation("rule_a", "msg one", "fn_a:1"),
            violation("rule_a", "msg two", "not-a-location"),
            violation("rule_a", "msg three", "fn_a:not-a-number"),
        ];
        // None of these are similar to each other (different messages, and
        // location parsing fails for the latter two), so all three survive.
        let result = dedupe_similar(violations);
        assert_eq!(result.len(), 3);
    }
}
