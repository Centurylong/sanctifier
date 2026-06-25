# `.sanctify.toml` Configuration Reference

Sanctifier reads its behaviour from a single TOML file named `.sanctify.toml`.
This page documents **every key**, its type, default value, how values are
resolved (precedence), and a fully annotated sample you can copy into a project.

It is part of the [core documentation set](README.md). If you are adding
Sanctifier to an existing project, start with the
[Migration Guide](migration.md); for command flags that interact with these
keys, see the [CLI Reference](cli.md).

---

## 1. Where the file lives and how it is found

You do not pass the config path on the command line. Instead, for every command
that needs configuration (`analyze`, `verify`, `callgraph`), Sanctifier
**searches upward** from the path you are scanning:

1. Start in the directory of the scanned path (or the directory itself if you
   passed a directory).
2. Look for `.sanctify.toml` there.
3. If not found, move to the parent directory and repeat, up to the filesystem
   root.
4. The **first** `.sanctify.toml` found wins.
5. If none is found, Sanctifier falls back to its **built-in defaults** (the
   table below).

This means a single `.sanctify.toml` at the root of a workspace applies to every
contract crate beneath it, and a contract can override the workspace config by
placing its own `.sanctify.toml` closer to the source.

> **Tip:** Run [`sanctifier init`](cli.md#sanctifier-init) to drop a starter
> `.sanctify.toml` into the current directory. Use `--force` to overwrite an
> existing one.

A malformed file (invalid TOML, or types that do not match) is **silently
ignored** and the built-in defaults are used. Validate your file before relying
on it (see [Validating your configuration](#6-validating-your-configuration)).

---

## 2. Key reference

All keys are optional. Omitted keys fall back to the defaults shown below.

| Key | Type | Default | Applies to |
|-----|------|---------|------------|
| [`ignore_paths`](#ignore_paths) | array of strings | `["target", ".git"]` | `analyze`, `verify`, `callgraph` |
| [`enabled_rules`](#enabled_rules) | array of strings | `["auth_gaps", "panics", "arithmetic", "ledger_size", "events"]` | `analyze` |
| [`ledger_limit`](#ledger_limit) | integer (bytes) | `64000` | `analyze` |
| [`approaching_threshold`](#approaching_threshold) | float (0.0–1.0) | `0.8` | `analyze` |
| [`strict_mode`](#strict_mode) | boolean | `false` | `analyze` |
| [`custom_rules`](#custom_rules) | array of tables | `[]` (none) | `analyze` |

---

### `ignore_paths`

**Type:** array of strings · **Default:** `["target", ".git"]`

Directory name fragments that Sanctifier skips while walking the source tree.
Matching is a **substring match against each directory name** (not a glob and
not a full path), so `"target"` skips any directory whose name contains
`target`. Use it to keep build artifacts, vendored code, and snapshot fixtures
out of the scan.

```toml
ignore_paths = ["target", ".git", "test_snapshots", "node_modules"]
```

---

### `enabled_rules`

**Type:** array of strings · **Default:** `["auth_gaps", "panics", "arithmetic", "ledger_size", "events"]`

Records which built-in detector families you intend to run. The recognised
identifiers are:

| Identifier | Detector family | Finding code |
|------------|-----------------|--------------|
| `auth_gaps` | Missing `require_auth` on state-mutating functions | [`S001`](error-codes.md) |
| `panics` | `panic!` / `unwrap` / `expect` usage | `S002` |
| `arithmetic` | Unchecked arithmetic (overflow/underflow) | `S003` |
| `ledger_size` | Ledger entry size limits | `S004` |
| `events` | Inconsistent event topic counts / gas patterns | `S008` |
| `invariants` | Declared `#[sanctify::invariant]` checks (see [`verify`](cli.md#sanctifier-verify)) | `S011` |

> **Behaviour note (read this):** the built-in analyzer currently runs its full
> detector set regardless of this list — `enabled_rules` is **declarative
> intent** that [`sanctifier init`](cli.md#sanctifier-init) writes and validates
> (it must be non-empty), and is reserved for per-rule gating. If you need to
> *guarantee* a rule runs today, [`custom_rules`](#custom_rules) always execute.
> Do not rely on removing an entry here to disable a built-in detector.

---

### `ledger_limit`

**Type:** integer (bytes) · **Default:** `64000`

The ledger-entry size budget, in bytes, used by the ledger-size detector
([`S004`](error-codes.md)). Contract state estimated at or above this value is
flagged as exceeding the limit. The default mirrors the Soroban ledger entry
size limit.

**Precedence:** the [`analyze --limit <bytes>`](cli.md#sanctifier-analyze) flag
**overrides** this key for a single run. The CLI flag itself defaults to
`64000`, so if you set `ledger_limit` in the file *and* do not pass `--limit`,
the file value is used.

```text
analyze --limit  (if passed)   ->  wins
.sanctify.toml ledger_limit     ->  used when --limit is absent
built-in default (64000)        ->  used when neither is set
```

---

### `approaching_threshold`

**Type:** float between `0.0` and `1.0` · **Default:** `0.8`

The fraction of [`ledger_limit`](#ledger_limit) at which Sanctifier raises an
**“approaching limit”** warning instead of a hard failure. With the defaults,
state estimated at `0.8 × 64000 = 51200` bytes or more (but below the limit) is
flagged as approaching. Lower it to get earlier warnings; raise it to reduce
noise.

```toml
approaching_threshold = 0.75
```

---

### `strict_mode`

**Type:** boolean · **Default:** `false`

When `true`, the ledger-size detector becomes more conservative: state estimated
at **90% of [`ledger_limit`](#ledger_limit) or above** is treated as *exceeding*
the limit (not merely approaching). Use it in CI to fail builds before they get
dangerously close to the cap.

```toml
strict_mode = true
```

---

### `custom_rules`

**Type:** array of tables (`[[custom_rules]]`) · **Default:** none

User-defined **regex** rules, evaluated against contract source during
`analyze`. Each match is reported under finding code
[`S007`](error-codes.md). Custom rules always run.

Each `[[custom_rules]]` table has:

| Field | Type | Required | Default | Notes |
|-------|------|----------|---------|-------|
| `name` | string | yes | — | Identifier shown in the report. |
| `pattern` | string | yes | — | A regular expression ([`regex` crate syntax](https://docs.rs/regex/latest/regex/#syntax)). Remember to escape backslashes in TOML. |
| `severity` | string | no | `warning` | One of `info`, `warning`, `error`. |

```toml
[[custom_rules]]
name = "no_unsafe_block"
pattern = "unsafe\\s*\\{"
severity = "error"

[[custom_rules]]
name = "no_mem_forget"
pattern = "std::mem::forget"
severity = "warning"
```

> **Escaping:** TOML basic strings treat `\` as an escape character, so a regex
> like `unsafe\s*\{` must be written `"unsafe\\s*\\{"`. Alternatively, use a
> TOML *literal* string with single quotes: `pattern = 'unsafe\s*\{'`.

---

## 3. Fully annotated sample

Copy this into `.sanctify.toml` at your workspace root and trim what you do not
need. Every key is shown with its default.

```toml
# .sanctify.toml — Sanctifier configuration
# Discovered by walking up from the scanned path; the first file found wins.

# Directory name fragments to skip while scanning (substring match).
ignore_paths = ["target", ".git", "test_snapshots"]

# Detector families you intend to run (see the configuration reference for the
# current behaviour note — built-in detectors run regardless today).
enabled_rules = ["auth_gaps", "panics", "arithmetic", "ledger_size", "events"]

# Ledger entry size budget in bytes. Overridden by `analyze --limit`.
ledger_limit = 64000

# Warn when estimated state reaches this fraction of `ledger_limit` (0.0–1.0).
approaching_threshold = 0.8

# When true, treat state >= 90% of `ledger_limit` as exceeding (good for CI).
strict_mode = false

# Optional regex rules. Each match is reported as S007.
[[custom_rules]]
name = "no_unsafe_block"
pattern = "unsafe\\s*\\{"
severity = "error"

[[custom_rules]]
name = "no_mem_forget"
pattern = "std::mem::forget"
severity = "warning"
```

---

## 4. Precedence summary

From highest to lowest priority:

1. **Command-line flags** for the current run (only `analyze --limit` overrides a
   config key today).
2. **The nearest `.sanctify.toml`** found by walking up from the scanned path.
3. **Built-in defaults** (the values in the [key reference](#2-key-reference)).

---

## 5. Minimal vs. complete configs

A minimal config that only customises ignored paths is perfectly valid — every
other key falls back to its default:

```toml
ignore_paths = ["target", ".git", "vendor"]
```

A stricter CI-oriented config:

```toml
ledger_limit = 64000
approaching_threshold = 0.7
strict_mode = true

[[custom_rules]]
name = "no_unwrap_in_prod"
pattern = '\.unwrap\(\)'
severity = "error"
```

---

## 6. Validating your configuration

Because a malformed `.sanctify.toml` is ignored in favour of defaults, confirm
your file parses and takes effect:

```bash
# Generate a fresh, known-good file to compare against.
sanctifier init            # writes .sanctify.toml (add --force to overwrite)

# Run a scan and confirm your custom rules / limits show up in the output.
sanctifier analyze . --format json | jq '.error_codes, .findings'
```

If a custom rule never fires or your `ledger_limit` seems ignored, check for a
TOML syntax error (a stray unescaped `\` in a regex is the usual culprit) and
verify there is not another `.sanctify.toml` higher up the tree shadowing the
one you edited.

---

## See also

- [CLI Reference](cli.md) — every command and flag, including `analyze --limit`.
- [Migration Guide](migration.md) — adding Sanctifier (and this file) to an existing repo.
- [FAQ & Troubleshooting](faq.md) — common configuration pitfalls.
- [Finding Codes](error-codes.md) — what `S001`…`S016` mean.
- [Glossary](glossary.md) — definitions for the security terms used above.
