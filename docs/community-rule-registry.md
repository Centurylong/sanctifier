# Community rule registry

This page is Sanctifier's canonical registry for **community-contributed static-analysis rules** that have completed maintainer review.

Today, community rules are normal in-tree Rust implementations of the `Rule` trait and are registered in `RuleRegistry`. Dynamic external plugins are a future direction documented in the [Detector Cookbook](detector-cookbook.md); an entry here does **not** imply that Sanctifier can load an arbitrary third-party package at runtime.

## Registry

| Rule | Finding code | Severity | Maintainer / author | Status | Source | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| _No accepted community entries yet._ | — | — | — | — | — | Add the first accepted entry in the same PR that lands the reviewed rule. |

Only rules merged into the Sanctifier repository may be marked **accepted**. Proposed rules belong in a GitHub issue or pull request, not in the accepted table.

## Registry entry contract

Each accepted row must include:

- **Rule** — the stable `Rule::name()` value.
- **Finding code** — the code allocated in `tooling/sanctifier-core/src/finding_codes.rs`.
- **Severity** — the default `Error`, `Warning`, or `Info` emitted by the rule.
- **Maintainer / author** — the contributor responsible for the original rule or the current maintainer when ownership transfers.
- **Status** — `accepted`, `deprecated`, or `superseded`.
- **Source** — a repository-relative link to the rule implementation.
- **Notes** — short compatibility, migration, or review notes when needed.

A registry row is documentation, not a second source of runtime configuration. Runtime registration remains in `RuleRegistry::with_default_rules()` until Sanctifier implements an external plugin interface.

## Contributing a community rule

1. **Open or select an issue.** Explain the bug pattern, why it matters for Soroban/Stellar code, and the expected false-positive boundary. For a new rule, reserve a finding code before publication.
2. **Follow the Detector Cookbook.** Implement the `Rule` trait under `tooling/sanctifier-core/src/rules/`, register it in `RuleRegistry::with_default_rules()`, and keep the rule deterministic.
3. **Add focused evidence.** Include a vulnerable fixture, a clean/negative case, the detector snapshot requested by the cookbook, and any narrowly necessary regression cases. Avoid unrelated test expansion.
4. **Document the finding.** Update the finding-code catalog and user-facing detector documentation when the rule introduces a new code or behavior.
5. **Add the registry row.** In the same pull request, replace or append a row in the registry above using the entry contract. New entries start as `accepted` only when the implementation is merged; before merge, the PR diff is the proposed registry update.
6. **Submit one focused PR.** Link the issue, describe the detection boundary and known limitations, and include the exact validation commands that were actually run.

Use the existing [Detector Cookbook](detector-cookbook.md) for implementation examples and [Contributing Guide](../CONTRIBUTING.md) for repository-wide build, test, and pull-request conventions.

## Review process

Maintainers review community rules on five dimensions:

1. **Security relevance and scope** — the rule must detect a concrete, explainable class of risk or code-quality failure without silently broadening into unrelated policy.
2. **Detection quality** — vulnerable cases are caught, representative safe cases remain clean, and obvious false-positive/false-negative boundaries are documented.
3. **Determinism and cost** — results must be reproducible; pathological scans, unbounded work, network dependencies, or hidden nondeterminism are not accepted in the core rule path.
4. **Evidence quality** — focused tests/snapshots cover the rule contract. Review should not rely on a large unrelated test matrix as a substitute for issue-specific evidence.
5. **Registry and docs accuracy** — finding code, default severity, source link, status, limitations, and contributor attribution must match the implementation.

A maintainer may request changes, reject a rule that duplicates an existing detector, or ask that a broad proposal be split before acceptance.

### Status changes

- **accepted** — implementation is merged and registered in the default in-tree registry.
- **deprecated** — still present for compatibility but contributors should not depend on it for new work; the notes column must name the replacement or reason.
- **superseded** — functionality has moved to another rule; the replacement must be linked.

Deprecating or superseding an entry should update both this registry and the runtime/docs surface in the same PR when practical.

## Security and trust boundary

Registry inclusion is **not** an endorsement of arbitrary external binaries, packages, URLs, or generated code. Contributors must not require reviewers or users to execute opaque artifacts, disclose credentials/private prompts, or fetch unpinned code as part of rule evaluation. The accepted implementation must remain reviewable in this repository under the project's normal contribution and security policies.
