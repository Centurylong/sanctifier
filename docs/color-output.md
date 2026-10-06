# Color output

Sanctifier keeps interactive output readable while allowing CI and log consumers to force plain text.

## Environment controls

- `NO_COLOR`: if the variable is present, styling is disabled regardless of its value.
- `CI`: truthy values disable styling for log-safe output. Empty, `0`, `false`, `no`, and `off` are treated as false.
- `SANCTIFIER_THEME`: `classic` keeps the normal automatic terminal behavior. `plain`, `mono`, or `monochrome` disables styling globally.

When none of those controls request plain output, Sanctifier leaves color selection to the `colored` crate's normal terminal detection.

## Examples

Disable color explicitly:

```bash
NO_COLOR=1 sanctifier analyze .
```

Disable color in CI:

```bash
CI=true sanctifier analyze .
```

Keep normal interactive color even when a tool defines a false CI value:

```bash
CI=false sanctifier analyze .
```

Select a plain theme:

```bash
SANCTIFIER_THEME=plain sanctifier analyze .
```

Use the default classic behavior:

```bash
SANCTIFIER_THEME=classic sanctifier analyze .
```
