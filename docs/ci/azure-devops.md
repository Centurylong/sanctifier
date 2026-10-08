# Azure DevOps: Sanctifier scan with downloadable SARIF

Use this integration to make Sanctifier findings visible on Azure Pipelines
builds and PR checks. It produces **both** the original machine-readable JSON
and a SARIF 2.1.0 report without requiring a third-party Python package or
Azure marketplace extension.

## Install in a Soroban repository

1. Copy [azure-pipelines.yml](../../ci/azure/azure-pipelines.yml) to
   `azure-pipelines.yml` at your target repository's root.
2. Copy [sanctifier_to_sarif.py](../../ci/azure/sanctifier_to_sarif.py) to
   `ci/azure/sanctifier_to_sarif.py` in that same target repo. Keep its path
   aligned with the command in the YAML. Both files must be committed.
3. Set `variables.sanctifierScanPath` to the Soroban contract directory (or
   workspace) containing its `Cargo.toml`. The default `.` is appropriate
   only if your repository root is a valid Soroban project.
4. In Azure DevOps, create a **Pipeline** from that repository's
   `azure-pipelines.yml`. Enable pull-request build validation for the target
   branch according to the repository's Azure Repos/GitHub trigger settings.

The full copy-paste job (installation, scanning, conversion and publication)
is in [azure-pipelines.yml](../../ci/azure/azure-pipelines.yml). The key steps:

```yaml
variables:
  sanctifierScanPath: 'contracts/my-token'

steps:
  # ... checkout and pinned Rust/dependency installation from example ...
  - bash: |
      set -euo pipefail
      report_dir="$(Build.ArtifactStagingDirectory)/sanctifier"
      mkdir -p "$report_dir"
      set +e
      sanctifier analyze "$(sanctifierScanPath)" --format json > "$report_dir/findings.json"
      scan_status=$?
      set -e
      python3 ci/azure/sanctifier_to_sarif.py \
        "$report_dir/findings.json" "$report_dir/results.sarif"
      exit "$scan_status"
    displayName: Scan and generate SARIF

  - task: PublishPipelineArtifact@1
    condition: succeededOrFailed()
    inputs:
      targetPath: '$(Build.ArtifactStagingDirectory)/sanctifier'
      artifact: sanctifier-analysis
      publishLocation: pipeline
```

The example pins Rust **1.85.0** and installs `clang`,
`libclang-dev`, `libz3-dev`, `libdbus-1-dev`, and `pkg-config` to match
Sanctifier's build prerequisites. The CLI declares an **independent nested
Cargo workspace** at `tooling/sanctifier-cli`; it is not a member of the
repository-root Cargo workspace. The pipeline therefore clones Sanctifier
into the Azure agent's temporary directory, then runs
`cargo +1.85.0 install --locked --path "$sanctifier_src/tooling/sanctifier-cli"`.
Installing the package with `cargo install --git ... sanctifier-cli` can
miss that nested workspace. The executable still lands in Cargo's bin directory
available to later steps on the agent.

CI runners need network access and permission to install build dependencies.
The sample shallow-clones the upstream default branch; for reproducible
production builds, check out a reviewed immutable upstream commit before the
`cargo install --path` command or vendor a known CLI binary.

## Results and failure behavior

- **Scan output:** `sanctifier-analysis/findings.json`, in Sanctifier's
  real `analyze --format json` schema.
- **SARIF:** `sanctifier-analysis/results.sarif`, standard SARIF 2.1.0,
  downloadable from the pipeline run's published artifact.
- **Gate:** high/critical findings retain the CLI's nonzero exit status.
  The artifact publishing step uses `succeededOrFailed()` so a completed
  scan still publishes evidence when findings fail the check.
- **Locations:** only findings with a known repository-relative `.rs` file
  **and** positive line number receive SARIF code locations; do not invent
  locations for summary-only entries.
- **Severity:** critical/high map to SARIF `error`, medium to `warning`,
  low/info to `note`; other findings use `warning`.
- **Azure UI:** the built-in task publishes a downloadable SARIF artifact.
  It does **not** automatically populate Azure DevOps Advanced Security,
  Code Analysis, or PR inline annotations; those require a separately
  configured compatible ingestion extension/service.

The generator intentionally reads `sanctifier analyze --format json` instead
of relying on `sanctifier ci --format sarif`: current `ci` prints a
human-readable banner, and its analyzer only recognizes `json` as the JSON
output path. Redirecting `ci --format sarif` would not produce valid SARIF.

## Focused offline smoke check

Run this in a checkout containing the copied converter. It exercises both a
located finding and one without an exact source location, including the
artifact's required SARIF envelope:

```bash
python3 - <<'PY'
import json, tempfile, subprocess
from pathlib import Path

with tempfile.TemporaryDirectory() as directory:
    p = Path(directory)
    (p / "input.json").write_text(json.dumps({"findings": {
        "panic_issues": [
            {"code": "S003", "function_name": "mint",
             "location": "src/lib.rs:42"}
        ],
        "auth_gaps": [{"code": "S001", "function": "transfer"}],
    }}))
    subprocess.run(["python3", "ci/azure/sanctifier_to_sarif.py",
                    str(p / "input.json"), str(p / "out.sarif")],
                   check=True)
    sarif = json.loads((p / "out.sarif").read_text())
    assert sarif["version"] == "2.1.0"
    results = sarif["runs"][0]["results"]
    assert len(results) == 2
    located = next(r for r in results if r["ruleId"] == "S003")
    assert located["locations"][0]["physicalLocation"]["region"]["startLine"] == 42
    unlocated = next(r for r in results if r["ruleId"] == "S001")
    assert "locations" not in unlocated
    print("Azure SARIF adapter fixture smoke: PASS")
PY
```

This local check covers JSON-to-SARIF behavior, **not** an execution on hosted
Azure DevOps; a live Azure pipeline run is needed to verify its runner image,
CLI installation, artifact publication and PR trigger wiring.
