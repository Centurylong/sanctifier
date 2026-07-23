# Custom Rules API

Sanctifier allows third-party developers to write and register custom static analysis detectors using a stable `Rule` trait.

## Example Usage

```rust
use sanctifier_core::rules::{Rule, Registry};

let mut registry = Registry::new();
registry.register(MyCustomRule);
```
