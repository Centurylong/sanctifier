# `map_iteration_order` — Map iteration-order dependence

| | |
| --- | --- |
| **Finding code** | [`SANCT_MAP_ITERATION_ORDER`](../error-codes.md) |
| **Category** | determinism |
| **Severity** | Warning |
| **Source rule** | [`rules/map_iteration_order.rs`](../../tooling/sanctifier-core/src/rules/map_iteration_order.rs) |

## What it catches

Selection logic whose observable result depends on the iteration order of a
Soroban `Map`. Examples include taking the first/last/nth item, using `find`
or `find_map`, returning a loop item from the first matching iteration, or
storing a loop item and then breaking.

The finding is advisory. The rule intentionally does not warn just because a
Map is traversed: complete traversals used for order-independent aggregation
are left alone.

## Vulnerable example

```rust
pub fn pick_large(entries: Map<Address, i128>) -> Option<Address> {
    for (key, value) in entries.iter() {
        if value > 10 {
            return Some(key);
        }
    }
    None
}
```

If multiple entries satisfy the predicate, whichever one is visited first
becomes the result.

## The fix

Define the ordering requirement explicitly before selecting an item, or avoid
selection by iteration entirely:

```rust
pub fn read_known(entries: Map<Address, i128>, key: Address) -> Option<i128> {
    entries.get(key)
}
```

When the business rule genuinely means "smallest key", "highest score", or
another ordered choice, collect/compare using that explicit stable key rather
than treating collection iteration order as policy.

## How Sanctifier detects it

The rule tracks local bindings and function arguments whose type is `Map`
(or whose initializer is a `Map::...` constructor). It reports two narrow
classes of syntax:

1. order-selecting iterator methods such as `next`, `last`, `nth`, `find`,
   `find_map`, and `position`;
2. `for` loops over `Map::iter`/`keys`/`values` that return a loop-bound item,
   or assign a loop-bound item to outer state and then break.

Simple iterator adapters such as `filter`, `map`, `skip`, and `take` are traced
back to the originating Map. Full traversal without an order-selected outcome
is not reported, so commutative aggregation and direct keyed lookup remain
quiet.

**Limitations:** this is a syntactic advisory, not whole-program data-flow
analysis. It does not infer Map-ness through arbitrary helper-return values or
aliases, and it does not attempt to prove whether a custom fold/reduction is
mathematically order-sensitive.

## References

- [Soroban SDK `Map`](https://docs.rs/soroban-sdk/latest/soroban_sdk/struct.Map.html)
- Related: [`arg_dos`](arg_dos.md), [`unbounded_storage`](unbounded_storage.md)
