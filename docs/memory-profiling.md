# Memory Profiling

Sanctifier profiles peak RSS (Resident Set Size) during analysis to help teams size their CI runners and detect memory regressions.

## Quick start

```bash
# Profile a project and print memory stats
sanctifier analyze . --profile

# Enforce a hard memory cap (aborts if peak RSS exceeds limit)
sanctifier analyze . --max-memory 512

# Both: profile AND enforce a limit
sanctifier analyze . --profile --max-memory 1024
```

## Sample output

```
📊 Memory (start): 12 MB RSS
📊 Memory (after collection): 87 MB RSS (peak: 87 MB)
📊 Memory (after suppression): 92 MB RSS (peak: 92 MB)
📊 Memory (final): 95 MB RSS (peak: 95 MB)
✨ Static analysis complete.
```

## How it works

- **Linux**: reads `VmRSS` from `/proc/self/status` — no external dependencies.
- **macOS / Windows**: fallback to `0` (tracking not available). The `--profile` flag still works but reports no data.
- **--max-memory**: sanitifier samples RSS after file collection and after suppression. If peak RSS exceeds the limit, the scan aborts with exit code 1.

## Expected limits

Peak memory usage depends on file count and finding density:

| Files | Findings / file | Peak RSS (approx) |
|-------|-----------------|--------------------|
| 100   | 5               | ~50 MB             |
| 500   | 10              | ~200 MB            |
| 2,000 | 15              | ~500 MB            |
| 5,000 | 20              | ~1.2 GB            |

These are estimates for a typical Soroban monorepo. Actual usage varies with struct/enum count, macro expansion complexity, and baseline size.

Measure with `--profile` on your own codebase for accurate numbers.

## CI integration

```yaml
# .github/workflows/ci.yml — add a max-memory step
- name: Sanctifier security scan
  run: sanctifier analyze . --max-memory 1024 --format json > sanctifier-report.json
```

The scan aborts before OOM-killing the runner, giving you a clean error message instead of a cryptic `137` exit code.

## Reduction strategies

If your project hits the limit:

1. **Split the workspace**: analyze sub-crates individually with `sanctifier analyze ./packages/foo`.
2. **Tune ignore paths**: add `target/`, `node_modules/`, and generated code to `.sanctify.toml` > `ignore_paths`.
3. **Use --no-baseline**: skipping baseline comparison reduces post-suppression memory by ~15%.
4. **Run in text mode**: `--format text` avoids the JSON object-tree overhead.

## Profiling your own detector

If you're writing a new detector, benchmark its per-file memory cost:

```bash
# Run the benchmark suite (criterion)
cargo bench -p sanctifier-core

# Profile with valgrind/massif
cargo build -p sanctifier-cli --release
valgrind --tool=massif --massif-out-file=massif.out \
  ./tooling/sanctifier-cli/target/release/sanctifier analyze ./contracts/amm-pool
ms_print massif.out
```
