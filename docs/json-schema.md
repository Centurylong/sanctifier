# Sanctifier JSON Report Schema (`v1` / `1.0.0`)

Sanctifier publishes a versioned JSON Schema (Draft 2020-12) for the machine-readable output emitted by `sanctifier analyze --format json` and `sanctifier ci --format json`.

- **Published schema file**: [`schemas/sanctifier-report.v1.schema.json`](../schemas/sanctifier-report.v1.schema.json)
- **Canonical `$id` / `$schema`**: `https://sanctifier.dev/schemas/sanctifier-report.v1.schema.json`
- **Current `schema_version`**: `"1.0.0"`

## Exporting the Schema from the CLI

You can print the embedded JSON Schema directly from the `sanctifier` binary:

```bash
sanctifier analyze --print-schema > sanctifier-report.v1.schema.json
```

## Versioning Policy

- **Patch / Minor (`1.x.y`)**: Additive, backward-compatible fields may be introduced without breaking existing consumers.
- **Major (`2.0.0+`)**: Any removal, rename, or incompatible type change to required properties increments the major schema version and publishes a new `schemas/sanctifier-report.vN.schema.json` artifact.

## CI Validation

The test suite in `tooling/sanctifier-cli/tests/cli_tests.rs` and `sanctifier_core::report_schema::validate_report_json_v1` continuously verify that `sanctifier analyze --format json` conforms to `schemas/sanctifier-report.v1.schema.json` and that `sanctifier analyze --print-schema` stays in lockstep with the published schema file.
