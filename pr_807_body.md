Closes #807

**Summary of Changes**:
Implemented the `sanctifier ci` subcommand to serve as the canonical entry point for Continuous Integration gates. This ensures that CI environments have a standardized, unified command for running analysis pipelines, resolving previous inconsistencies in CI gating.

**What changed**:
* `tooling/sanctifier-cli/src/main.rs`: Added the `ci` subcommand to the CLI parser.
* `tooling/sanctifier-cli/src/commands/ci.rs`: Implemented the execution logic for the `ci` command, integrating it with the underlying analysis engine.

**Testing / Local Verification**:
* Verified that the `sanctifier ci` subcommand triggers the correct execution path locally.
