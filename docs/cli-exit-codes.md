# CLI exit codes and analysis summary

Sanctifier exposes a small, documented process-code contract for shell scripts and CI.

| Exit code | Meaning |
| --- | --- |
| `0` | The command completed successfully. Text-mode `analyze` keeps its existing report-only behavior, so findings do not by themselves change this code. |
| `1` | The command ran but returned a failing result or encountered a general operational error. Existing examples include high/critical findings from `analyze --format json`, `diff --fail-on-new` detecting regressions, `verify --strict` failing an invariant, WASM error findings, and an invalid analysis target. |
| `2` | The invocation or a required command-specific prerequisite/protocol prevented a valid result. Clap uses this for unknown options and missing or invalid arguments; commands may also use it for an unavailable required helper or malformed helper output (for example, `audit` when `cargo-audit` is unavailable or does not return a parseable report). |

Scripts should treat every non-zero value as failure unless they intentionally distinguish a completed failing result (`1`) from an invocation or prerequisite failure (`2`).

## Text analysis summary

After a completed text-mode `sanctifier analyze` run, stdout ends with one uncolored, machine-readable line:

```text
SANCTIFIER_SUMMARY exit_code=0 total_findings=<n> suppressed_findings=<n> has_critical=<true|false> has_high=<true|false>
```

Every field after `SANCTIFIER_SUMMARY` is a whitespace-delimited `key=value` token whose value contains no spaces. The `exit_code` field is the process code for that completed text-mode scan.

Operational failures that stop analysis before a report is produced do not emit a completion summary.

## JSON analysis summary

JSON mode remains one valid JSON document. Its existing `summary` object now includes:

- `exit_code`
- `total_findings`
- `suppressed_findings`
- `has_critical`
- `has_high`

The JSON `exit_code` matches the process code used by the existing JSON high/critical finding gate.
