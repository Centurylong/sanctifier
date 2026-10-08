# CSV finding export

`sanctifier analyze --format csv` writes RFC 4180-style CSV to standard output for spreadsheet and BI ingestion. Diagnostic/progress output stays on standard error so stdout can be redirected directly to a file.

## Stable schema

The first row is always:

```text
schema_version,category,code,location,summary,details_json
```

| Column | Contract |
| --- | --- |
| `schema_version` | Literal `sanctifier-csv-v1` for every finding row. |
| `category` | Stable finding collection name such as `auth_gaps`, `panic_issues`, or `vulnerability_db_matches`. |
| `code` | Sanctifier finding code, or the vulnerability-database ID for database matches. |
| `location` | Best available source location; empty when the detector has no location. |
| `summary` | Short human-readable finding summary. |
| `details_json` | JSON serialization of the complete finding payload for lossless downstream processing. |

Fields containing commas, double quotes, carriage returns, or newlines are quoted. Embedded double quotes are doubled. Rows use CRLF line endings.

### Spreadsheet-safe text cells

Finding locations, summaries and other scalar cells can contain untrusted source
text. Spreadsheet software may interpret a cell beginning with `=`, `+`, `-` or
`@` as a formula even when the CSV cell is correctly quoted. For spreadsheet
consumption, the exporter prefixes these cells with a literal apostrophe;
leading whitespace (and a possible byte-order mark) is ignored when detecting
that prefix. The apostrophe goes *inside* CSV quotes, so commas and newlines
still parse as a single cell. Normal fields are unaffected.

The scalar columns are presentation text. If a prefixed value must be recovered
verbatim for machine processing, use the original, unmodified finding in the
`details_json` column. This mitigates common spreadsheet formula interpretation
but is not a guarantee about every spreadsheet product or import setting.

The schema version changes only when the column contract changes incompatibly. Consumers should key parsing behavior on `schema_version`, not on the Sanctifier package version.
