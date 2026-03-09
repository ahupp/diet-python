---
name: run-pystone-benchmark
description: Run this repo's pystone benchmark script and capture the output in logs/. Use when Codex needs to benchmark transformed or JIT execution against stock CPython, compare loops per second, or summarize the throughput reported by scripts/benchmark.sh.
---

# Run Pystone Benchmark

Use `scripts/benchmark.sh` from the repo root and capture the output to a file in `logs/`.

## Run

Use `set -o pipefail` so the benchmark exit status is preserved when logging:

```bash
set -o pipefail
./scripts/benchmark.sh 1000000 2>&1 | tee logs/benchmark_run.log
```

If the user requests a different loop count, pass it as the first argument and keep the log in `logs/`.

## Summarize

The script prints two sections:

- `jit transformed`
- `stock cpython`

For each run, report:

- transformed or JIT loops per second
- stock loops per second
- relative slowdown or speedup factor

If the user asks for a warmed comparison, note that `scripts/benchmark.sh` is a cold run wrapper. Use the perf profiling workflow or an explicit same-process warmup if they need steady-state numbers.

## Notes

- Build output may appear before the benchmark numbers when the release extension is stale.
- Keep benchmark artifacts in `logs/` and refer to the log path in the final summary.
