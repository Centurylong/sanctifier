//! Detector documentation coverage (issue #767).
//!
//! Enforces that the per-detector documentation catalog under `docs/detectors/`
//! stays in lock-step with the detectors actually registered in
//! `RuleRegistry::with_default_rules()`. This is the "CI checks coverage"
//! acceptance criterion: it runs inside the existing `cargo test -p
//! sanctifier-core` step, so adding a detector without documenting it (or
//! leaving an orphan page after removing a detector) fails the build.
//!
//! To add a detector:
//!   1. Register it in `src/rules/mod.rs`.
//!   2. Create `docs/detectors/<detector_name>.md`.
//!   3. Add a row for it to `docs/detectors/README.md`.

use sanctifier_core::RuleRegistry;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Absolute path to the repository's `docs/detectors/` directory, derived from
/// this crate's manifest dir (`tooling/sanctifier-core`) so the test is
/// independent of the current working directory.
fn detectors_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("detectors")
}

/// The detector names registered in the default registry.
fn registered_detectors() -> BTreeSet<String> {
    RuleRegistry::with_default_rules()
        .available_rules()
        .into_iter()
        .map(|s| s.to_string())
        .collect()
}

/// The detector page stems present on disk (every `*.md` except the `README`
/// index).
fn documented_detectors() -> BTreeSet<String> {
    let dir = detectors_dir();
    let mut pages = BTreeSet::new();
    for entry in
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("utf-8 file stem")
            .to_string();
        if stem.eq_ignore_ascii_case("README") {
            continue;
        }
        pages.insert(stem);
    }
    pages
}

#[test]
fn every_detector_has_a_documentation_page() {
    let registered = registered_detectors();
    let documented = documented_detectors();

    let missing: Vec<&String> = registered.difference(&documented).collect();
    assert!(
        missing.is_empty(),
        "detectors registered but missing docs/detectors/<name>.md: {missing:?}. \
         Create the page(s) and add them to docs/detectors/README.md."
    );
}

#[test]
fn no_orphan_detector_pages() {
    let registered = registered_detectors();
    let documented = documented_detectors();

    let orphans: Vec<&String> = documented.difference(&registered).collect();
    assert!(
        orphans.is_empty(),
        "docs/detectors/ pages with no corresponding registered detector: {orphans:?}. \
         Remove the page or register the detector."
    );
}

#[test]
fn index_lists_every_detector() {
    let index = detectors_dir().join("README.md");
    let body = std::fs::read_to_string(&index)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", index.display()));

    for name in registered_detectors() {
        // Each detector must be linked from the catalog index, e.g. `(auth_gap.md)`.
        let needle = format!("{name}.md");
        assert!(
            body.contains(&needle),
            "docs/detectors/README.md does not link to `{needle}`. \
             Add a table row for the `{name}` detector."
        );
    }
}
