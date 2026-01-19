# Plan


Python is a surprisingly complicated language; so we first need to make it a smaller language.  dp-transform lowers python to a subset of python with a much smaller featureset: 

 - functions
 - variables
 - while loops without "else" blocks
 - if stmt without elif/else blocks
 - try/except, async and yield.

In particular, this removes class definitions, lambda, generators, set/dict/list literals, unpacking, 
with / async with, operators, and f/t-strings.

For codegen, we want to expose as much 

# diet-python

This repository includes a small Rust utility for transforming Python source
code. It parses a file with Ruff's parser and rewrites binary operations and
augmented assignments (e.g., `+=`) into calls to the corresponding functions in
the standard library's `operator` module. The transformation is idempotent, so
re-running it on already rewritten code leaves the output unchanged.

TODO: Preserve PEP 695 type alias semantics (lazy evaluation) while still
transforming `type Alias = ...` statements; currently they are left unchanged.

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

# CLIF

```
$ rustup component add rustc-codegen-cranelift-preview --toolchain nightly
$ CARGO_PROFILE_DEV_CODEGEN_BACKEND=cranelift \
      cargo +nightly rustc -Zcodegen-backend -p soac-runtime --lib --   --emit=llvm-ir -Ccodegen-units=1

# or
$ ./rust-clif-dist/rustc-clif --out-dir=clif-out/ --crate-type=rlib fastadd.rs -Cdebuginfo=0 --emit link,llvm-ir
```

# Log

2026-01-15:
  - Totals: duration 18m 3s; tests run 37,414; failures 747; skipped 1,706; test files run 483/492; failed 103; env_changed 1;
    skipped 31; resource_denied 9
2026-01-16:
  - Test files: 401 passed / 492 total (483 run; 81 failed; 1 env_changed; 31 skipped; 9 resource_denied).
  - Test cases: 39,237 passed / 39,820 total (583 failed; 1,835 skipped).

Then 
• Test File Counts

  - Passing: 388/492
  - Run: 483/492
  - Failed: 95
  - Skipped files: 44
  
  Individual Test Cases

  - Run: 39,320
  - Passed: 38,685
  - Failed: 635
  - Skipped: 1,754

2026-01-17:
Total duration: 33 min 49 sec
Total tests: run=28,491 failures=612 skipped=1,426
Total test files: run=488/492 failed=160 skipped=24 resource_denied=4
Result: FAILURE
