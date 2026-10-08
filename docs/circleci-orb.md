# CircleCI integration (issue #708)

This integration uses a public CircleCI 2.1 **URL orb**. CircleCI can import
orb configuration from GitHub without requiring a registry publication.
Organizations must allow-list the source URL as described in the CircleCI
documentation: https://circleci.com/docs/orbs/use/orb-intro/

The orb source is `integrations/circleci/sanctifier-orb.yml`. It defines a reusable
`sanctifier/analyze` command and a ready-to-run `sanctifier/scan` job.

## Consumer configuration

Add the following to the consumer repository's `.circleci/config.yml`:

```yaml
version: 2.1
orbs:
  sanctifier: https://raw.githubusercontent.com/Centurylong/sanctifier/refs/heads/main/integrations/circleci/sanctifier-orb.yml

workflows:
  security:
    jobs:
      - sanctifier/scan:
          path: contracts/amm-pool
```

Until the upstream PR merges, replace the URL with the public development
branch at
`https://raw.githubusercontent.com/woahwhattheheck/sanctifier/refs/heads/gf708-circleci-url-orb-20261008/integrations/circleci/sanctifier-orb.yml`.
Pin to an immutable source commit for production configurations.

## Workflow behavior

1. The reusable job checks out the consumer repository in `cimg/rust:1.85`.
2. The job installs the default CLI's native build prerequisites (clang,
   pkg-config, libz3-dev, libdbus-1-dev) in the Rust 1.85 Linux image, then
   checks out the official Sanctifier source and installs the nested
   `tooling/sanctifier-cli` Cargo workspace with its committed lockfile.
   A source SHA pin is checked out before installation. A root-level
   `cargo install --git` does not discover this separately nested workspace.
3. The command runs `sanctifier analyze <path> --format json`. Its JSON output
   is written to `artifacts/sanctifier/report.json`; the original scan status
   is saved to `artifacts/sanctifier/exit-code`.
4. CircleCI `store_artifacts` runs even for a finding-triggered nonzero scan
   because the status is enforced in a **separate final step**. The scan job
   retains the original success/failure semantics after upload.

The command accepts `path`, `artifacts_dir`, and `git_revision` parameters.
Set `git_revision` to an audited upstream commit SHA to pin scanner behavior;
without it, Cargo installs the latest repository head. Override
`artifacts_dir` to change where the report and status are stored.

Reports are artifacts, not a covert bypass: a finding-triggered exit still
fails the job *after* upload. A failed dependency installation, Git checkout,
or Cargo install occurs before scanning and therefore cannot produce a
meaningful report; it must not be reported as a successful scan.

## Copy-paste smoke example

The example repo can be the existing Sanctifier project itself, with
`path: contracts/amm-pool`; its Soroban Cargo project is already tracked.
A consumer should enable GitHub-to-CircleCI checkout, allow-list its URL-orb
source, and run the `security` workflow. Inspect CircleCI Artifacts for
`sanctifier/report.json` and `sanctifier/exit-code`; a finding must still
fail the scan job after those artifacts appear.

The repository also includes a standalone consumer config at
`integrations/circleci/example.config.yml`.

**Verification boundary:** This PR delivers the importable source and example.
A hosted CircleCI workflow execution and optional publication into a named
CircleCI registry namespace need an authorized CircleCI project and namespace;
do not treat example source as a completed hosted run.

See also:
- CircleCI configuration reference: https://circleci.com/docs/reference/configuration-reference/
- CircleCI URL orbs: https://circleci.com/docs/orbs/use/orb-intro/
- Sanctifier CLI reference: ../docs/cli.md
