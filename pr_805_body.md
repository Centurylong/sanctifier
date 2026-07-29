Closes #805

**Summary of Changes**:
Implemented a static analysis detector (`S025` - `MISSING_ADMIN_AUTH`) designed to flag public functions that update critical contract parameters (such as `fee`, `treasury`, `reserve`, `limit`, or `admin`) without invoking `require_auth()` or `require_auth_for_args()`. 

**What changed**:
* `tooling/sanctifier-core/src/rules/missing_auth.rs`: Added the detector logic which traverses the AST to find vulnerable parameter updates lacking authorization checks.
* `tooling/sanctifier-core/src/rules/mod.rs`: Registered the new rule.
* `tooling/sanctifier-core/src/finding_codes.rs`: Added the `S025` code.
* `tooling/sanctifier-core/tests/fixtures/detectors/missing_auth.rs`: Comprehensive tests covering missing auth vulnerabilities and safe, authenticated parameter updates.
* Generated golden snapshots for the new rule.

**Testing / Local Verification**:
* `cargo clippy --fix -p sanctifier-core` and `cargo fmt` resolved successfully.
* Passed snapshot tests validating both positive and negative cases.
