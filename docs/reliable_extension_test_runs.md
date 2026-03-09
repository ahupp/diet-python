# Reliable Extension Test Runs

This document describes the exact steps for a trustworthy test run when the
Python extension module (`diet_python.so`) may exist in multiple build outputs.

## Why this matters

`diet_import_hook._load_pyo3_extension()` can fall back to scanning
`target/debug` and `target/release` if `diet_python` was not already imported
from an explicit path. That can make a test run pick up the wrong artifact.

The safe pattern is:

1. build exactly the artifact you want
2. stage it in a fresh temporary directory as `diet_python.so`
3. prepend that directory to `PYTHONPATH`
4. verify `diet_python.__file__` before trusting the run

Do not rely on implicit extension discovery for debugging or release repros.

## Debug build: exact steps

```bash
cd /home/adam/project/diet-python

cargo build

STAGING_DIR="$(mktemp -d)"
ln -sf "$PWD/target/debug/libdiet_python.so" "$STAGING_DIR/diet_python.so"

PYTHONPATH="$STAGING_DIR:$PWD${PYTHONPATH:+:$PYTHONPATH}" \
PYTHONDONTWRITEBYTECODE=1 \
DIET_PYTHON_INTEGRATION_ONLY=1 \
./.venv-cpython/bin/python - <<'PY'
import diet_python, inspect
print(diet_python.__file__)
print(inspect.signature(diet_python.register_clif_vectorcall))
PY
```

Expected:

- `diet_python.__file__` points at the staged `mktemp` directory
- not `target/debug/...`
- not `target/release/...`

Then run the targeted test in the same environment:

```bash
PYTHONPATH="$STAGING_DIR:$PWD${PYTHONPATH:+:$PYTHONPATH}" \
PYTHONDONTWRITEBYTECODE=1 \
DIET_PYTHON_INTEGRATION_ONLY=1 \
./.venv-cpython/bin/python -m pytest --tb=native path/to/test.py -q
```

## Release build: exact steps

```bash
cd /home/adam/project/diet-python

cargo build --release

STAGING_DIR="$(mktemp -d)"
ln -sf "$PWD/target/release/libdiet_python.so" "$STAGING_DIR/diet_python.so"

PYTHONPATH="$STAGING_DIR:$PWD${PYTHONPATH:+:$PYTHONPATH}" \
PYTHONDONTWRITEBYTECODE=1 \
DIET_PYTHON_INTEGRATION_ONLY=1 \
./.venv-cpython/bin/python - <<'PY'
import diet_python, inspect
print(diet_python.__file__)
print(inspect.signature(diet_python.register_clif_vectorcall))
PY
```

Then run the target:

```bash
PYTHONPATH="$STAGING_DIR:$PWD${PYTHONPATH:+:$PYTHONPATH}" \
PYTHONDONTWRITEBYTECODE=1 \
DIET_PYTHON_INTEGRATION_ONLY=1 \
./.venv-cpython/bin/python -m pytest --tb=native \
tests/test_regression_async_with_error_message.py -q
```

## CPython suite runs

When invoking the vendored interpreter directly, use:

```bash
vendor/cpython/python -m pytest ...
```

or the `just` recipe:

```bash
just pytest-cpython tests/
```

If a CPython worker run was interrupted, clear stale workers before retrying:

```bash
pkill -f test.libregrtest.worker || true
```

If the test harness requires a tempdir:

```bash
--tempdir /tmp/<name>
```

## Mandatory sanity check

Before trusting any result, verify the imported extension path in the exact test
environment:

```bash
PYTHONPATH="$STAGING_DIR:$PWD${PYTHONPATH:+:$PYTHONPATH}" \
./.venv-cpython/bin/python - <<'PY'
import diet_python
print(diet_python.__file__)
PY
```

If that path is not the staged `diet_python.so`, the run is not trustworthy.

## Common failure mode

Bad pattern:

1. build one artifact
2. run Python without staging that exact `.so`
3. let import fallback choose whichever `target/.../libdiet_python.so` it finds

That can produce:

- debug/release mismatches
- stale crash repros
- confusing signature or ABI mismatches

Use the staged-path flow above instead.
