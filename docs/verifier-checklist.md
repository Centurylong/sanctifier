# Verifier checklist

A diffable checklist for reviewing a Soroban ZK-proof verifier contract
against the [reference template](verifier-reference-template.md). Each item
maps to a Sanctifier detector where one exists, so a checked box is
independently machine-checkable, not just a reviewer's opinion.

| # | Check | Reference template | Detector |
| - | --- | --- | --- |
| 1 | `init`/setup guards against re-initialization overwriting a live verifying key | `init` panics if `DataKey::VerifyingKey` already set | [`init_hardcoded_admin`](detectors/init_hardcoded_admin.md) (hardcoded values), manual review (re-init guard shape) |
| 2 | Every caller-supplied proof/public-input byte array is length-checked before being copied into a fixed-size buffer | `verify`'s `vk_slice`/`proof_slice` length checks | [`proof_length_check`](detectors/proof_length_check.md) |
| 3 | Public inputs are checked against the *exact* expected size for the circuit, not just "fits the buffer" | `public_inputs_bytes.len() != 4 * 32` | [`proof_length_check`](detectors/proof_length_check.md) |
| 4 | Deserialization failures return a clean rejection (`false`/`Err`), never `unwrap()`/`expect()` | `match ... { Ok(v) => v, Err(_) => return false }` | [`sanct_unwrap`](detectors/sanct_unwrap.md), [`view_panic`](detectors/view_panic.md) |
| 5 | `verify` is read-only (no storage write) — a proof check should never mutate state as a side effect | `verify` only reads `DataKey::VerifyingKey` | [`state_write_in_view`](detectors/state_write_in_view.md) |
| 6 | State-mutating entrypoints (e.g. rotating the verifying key) require `require_auth`/`require_auth_for_args`, not just an `init`-once guard | *(not applicable — reference template has no post-init mutator)* | [`auth_gap`](detectors/auth_gap.md), [`wrong_auth_args`](detectors/wrong_auth_args.md) |
| 7 | No hardcoded admin/verifying-key material outside of `init`'s parameter | `vk_bytes` is a formal `init` argument, never a literal | [`hardcoded_addr`](detectors/hardcoded_addr.md), [`init_hardcoded_admin`](detectors/init_hardcoded_admin.md) |

## How to use this

1. Diff your verifier contract against [`contracts/zk-verifier/src/lib.rs`](../contracts/zk-verifier/src/lib.rs).
2. For each row above, confirm your contract matches the reference's shape —
   or run `sanctifier scan` and confirm the mapped detector(s) are clean.
3. New verifier-specific findings should get a row here in the same PR that
   adds the detector (see [Detector Cookbook](detector-cookbook.md)).
