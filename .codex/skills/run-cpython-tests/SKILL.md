---
name: run-cpython-tests
description: Run the CPython regression test suite and generate structured summaries of the failures
---

# Run CPython tests

## Run the full suite

- Run with full permissions, including network.
- Use a 10-minute per-test timeout and keep parallelism enabled (run_cpython_tests.sh already uses -j0).
- Capture output to a single log file while preserving the exit status:

```bash
set -o pipefail
./scripts/run_cpython_tests.sh --timeout 180 2>&1 | tee logs/cpython_full_test_run.log
```

## Summarize failures from the log

- Locate failure anchors:

```bash
rg -n "^(FAIL|ERROR|TIMEOUT|CRASHED|INTERRUPTED|LEAKED|ENV_CHANGED):" cpython_full_test_run.log
```

- Extract each failure block (look for the separator lines of ===) and classify the failure based on the contents of the error.
- Use these categories in the summary:
  - FAIL: assertion mismatch or explicit test failure; call out the mismatched expectation.
  - ERROR: unexpected exception; report the exception type and message.
  - TIMEOUT: test exceeded 180 seconds; call out the hang/timeout.
  - CRASHED/FATAL: interpreter crash; mention the fatal error or signal.
  - LEAKED/ENV_CHANGED: resource leak or environment mutation; mention the resource.
- Summarize each failing test as: `test_name: <category> - <short reason from output>`.
- If a failure is due to changes to tracebacks / source transforms /
  bytecode, add it to EXPECTED_FAILURES.md with a short explanation
- Otherwise, add it to FAILURES_TO_TRIAGE.md

