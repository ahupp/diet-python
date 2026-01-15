---
name: summarize-cpython-failures
description: "Summarize CPython test failures from cpython_full_test_run.log (and cpython_full_test_run_summary.txt if present), compute passed/total counts for test files and test cases, and group failures by likely root cause. Use when asked to summarize or categorize failures from a CPython regrtest run in this repo."
---

# Summarize CPython test failures

## Workflow

1) Locate inputs
- Prefer `cpython_full_test_run.log` for raw failures.
- Use `cpython_full_test_run_summary.txt` for authoritative totals if the log lacks a final summary.

2) Compute totals
- Test files: parse regrtest progress lines like `[...] test_x passed/failed/skipped`.
- Test cases: parse `... ok|FAIL|ERROR|skipped|expected failure` lines.
- If summary totals exist, report both "executed" and "including skipped" counts and state the basis.

3) Extract failures
- From the log, collect each `FAIL:` / `ERROR:` block with its traceback.
- Capture test id + exception type + first error line.

4) Group by likely root cause
- Start with exception-type buckets (AssertionError, NameError/UnboundLocalError, TypeError, OSError/FileNotFoundError, UnicodeDecodeError, RecursionError, Connection errors).
- Split AssertionError into subgroups when messages indicate bytecode/line-number/traceback mismatches vs value mismatches.
- Note ambiguous cases explicitly.

5) Report
- Give totals first (passed/total files, passed/total test cases; clarify whether totals include skipped).
- List grouped failures; include each test file with the first error message and failure count.
- Call out non-standard failures (e.g., SyntaxError, RecursionError, UnicodeDecodeError, Connection errors).

## Handy parsing snippets

- File progress lines (supports `[x/y]` and `[x/y/z]`):
```python
import re, pathlib
log = pathlib.Path('cpython_full_test_run.log').read_text(errors='replace')
pat = re.compile(r"\[\s*(\d+)/(\d+)(?:/\d+)?\]\s+([^\s]+)\s+(passed|failed|skipped)")
files = {}
for line in log.splitlines():
    m = pat.search(line)
    if m:
        files[m.group(3)] = m.group(4)
```

- Test-case counts:
```python
import re, pathlib, collections
log = pathlib.Path('cpython_full_test_run.log').read_text(errors='replace')
case_re = re.compile(r"^[A-Za-z0-9_]+ \([^\)]+\) \.{3} (ok|FAIL|ERROR|skipped|expected failure|unexpected success)")
counts = collections.Counter(m.group(1) for m in map(case_re.match, log.splitlines()) if m)
```
