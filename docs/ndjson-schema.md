# NDJSON schema

`analyze --format ndjson` writes one JSON object per line and flushes each record as it is emitted. Every record contains:

- `schema`: currently `"sanctifier-ndjson-v1"`
- `type`: one of `"meta"`, `"finding"`, `"summary"`, or `"error"`
- `data`: the payload for that record type
- `category`: present only on `finding` records

## Record payloads

### `meta`

`data` contains `version`, `timestamp`, `project_path`, and `vulnerability_db_version`.

### `finding`

`category` identifies the finding collection. `data` always contains:

- `code`: the stable Sanctifier finding code (or vulnerability database ID)
- `finding`: the serialized finding payload for that category

Current categories are `storage_collisions`, `ledger_size_warnings`, `unsafe_patterns`, `auth_gaps`, `panic_issues`, `arithmetic_issues`, `custom_rules`, `event_issues`, `unhandled_results`, `upgrade_risks`, `smt_issues`, and `vulnerability_db_matches`.

### `summary`

`data` contains aggregate finding counts, `has_critical`, `has_high`, and a `baseline` object with `suppressed_count` and `stale_count`.

### `error`

`data` contains `success: false` and an `error` message. The CLI exits non-zero after emitting the record.
