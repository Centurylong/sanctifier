Closes #792

**Summary of Changes**:
Added a new static analysis detector (`wrong_auth_args`, S017) to flag missing explicit argument bindings during authorization in internal functions. It detects when internal helper functions use `.require_auth()` instead of `.require_auth_for_args()`, which leaves their specific arguments unbounded since `require_auth()` binds to the top-level contract invocation instead.

**What changed**:
* `tooling/sanctifier-core/src/rules/wrong_auth_args.rs`: Implemented the `WrongAuthArgsRule` which walks the AST and checks for `.require_auth()` calls within non-public functions.
* `tooling/sanctifier-core/src/rules/mod.rs`: Registered the new rule to the rule registry map.
* `tooling/sanctifier-core/src/finding_codes.rs`: Added the `S017` code for `UNBOUND_AUTH`.
* `tooling/sanctifier-core/tests/fixtures/detectors/wrong_auth_args.rs`: Added comprehensive test fixtures including both vulnerable internal functions and safe patterns (public endpoints and explicit `require_auth_for_args`).
* `tooling/sanctifier-core/tests/snapshots/detector_snapshots__wrong_auth_args.snap`: Generated the `insta` golden snapshot test covering the rule's functionality.

**Testing / Local Verification**:
* Executed `cargo clippy --fix -p sanctifier-core` to resolve all linting warnings.
* Ran `cargo fmt -p sanctifier-core` to ensure formatting compliance.
* Ran `INSTA_UPDATE=always cargo test -p sanctifier-core --test detector_snapshots` and confirmed the golden snapshot perfectly matches the expected behavior.
* Verified that public contract methods using `require_auth()` are not falsely flagged.
