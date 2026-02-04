---
name: run-cpython-tests
description: Run the CPython regression test suite and generate structured summaries of the failures
---

# Run CPython tests

## Run the full suite (transform mode)

- Run with full permissions, including network.
- Use a 3-minute per-test timeout and keep parallelism enabled (run_cpython_tests.sh already uses -j0).
- Capture output to a single log file while preserving the exit status:

```bash
set -o pipefail
./scripts/run_cpython_tests.sh --timeout 180 2>&1 | tee logs/cpython_full_test_run.log
```

## Run the fast suite (transform mode)

- Skip slow tests and keep parallelism enabled.
- Capture output to a log file while preserving the exit status:

```bash
set -o pipefail
./scripts/run_cpython_tests.sh -x slow 2>&1 | tee logs/cpython_fast_test_run.log
```

## Run the full suite (eval mode)

- Set DIET_PYTHON_MODE=eval to use the tree-walk evaluator.
- Capture output to a log file while preserving the exit status:

```bash
set -o pipefail
DIET_PYTHON_MODE=eval ./scripts/run_cpython_tests.sh --timeout 180 2>&1 | tee logs/cpython_full_eval_test_run.log
```

## Run the fast suite (eval mode)

- Skip slow tests and keep parallelism enabled.
- Capture output to a log file while preserving the exit status:

```bash
set -o pipefail
DIET_PYTHON_MODE=eval ./scripts/run_cpython_tests.sh -x slow 2>&1 | tee logs/cpython_fast_eval_test_run.log
```

## Run in transform-only mode explicitly (optional)

- Transform mode is the default; this is only needed if the environment is set differently.

```bash
set -o pipefail
DIET_PYTHON_MODE=transform ./scripts/run_cpython_tests.sh --timeout 180 2>&1 | tee logs/cpython_full_transform_test_run.log
```

## Summarize failures from the log

- Locate failure anchors:

```bash
rg -n "^(FAIL|ERROR|TIMEOUT|CRASHED|INTERRUPTED|LEAKED|ENV_CHANGED):" logs/cpython_full_test_run.log
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
