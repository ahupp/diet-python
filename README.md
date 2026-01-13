# diet-python

This repository includes a small Rust utility for transforming Python source
code. It parses a file with Ruff's parser and rewrites binary operations and
augmented assignments (e.g., `+=`) into calls to the corresponding functions in
the standard library's `operator` module. The transformation is idempotent, so
re-running it on already rewritten code leaves the output unchanged.

Run it with:

```
cargo run --bin diet-python -- path/to/file.py
```

## Python import hook

To apply the transform automatically when modules are imported, install the
provided import hook:

```python
import diet_import_hook
diet_import_hook.install()
```

After calling `install()`, any subsequent imports will be rewritten using the
`diet-python` transform before execution.

Run the included example to see the hook in action:

```
python example_usage.py
```

The script installs the hook, imports `example_module`, and asserts that its
bytecode calls `operator.add` instead of using `BINARY_OP`.

## Regenerating transform fixtures

If a transform change updates the expected desugaring, regenerate the fixture
outputs with:

```
cargo run --bin regen_fixtures
```

## CPython test suite

To run the official CPython test suite with a Python installed via `uv`, use:

```
scripts/run_cpython_tests.sh
```

The script clones the `cpython` repository if necessary, creates a virtual
environment using `uv`, and executes the test suite with that interpreter.


# Log

2026-01-15:
  - Totals: duration 18m 3s; tests run 37,414; failures 747; skipped 1,706; test files run 483/492; failed 103; env_changed 1;
    skipped 31; resource_denied 9
