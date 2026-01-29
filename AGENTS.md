# AGENTS
- **MUST FOLLOW**: Never add new variants to the minimal AST unless explicitly asked.
- **MUST FOLLOW**: Always run `cargo test` and `./scripts/pytest_cpython.sh tests/` before submitting changes.
- **MUST FOLLOW**: Always preserve behavior in the transformed code, particularly evaluation order.
- **MUST FOLLOW**: When traversing the AST, always use an impl of `crate::transformer::Transformer`.
- **NOTE**: Prefer adding behavior at transform time rather than runtime in `__dp__.py` whenever possible.
- **MUST FOLLOW**: If a fixture error occurs, regenerate all fixtures by running `cargo run --bin regen_snapshots` with no file arguments.
- **NOTE**: Use `cargo run --bin regen_snapshots` to regenerate fixtures instead of manual edits.
- **NOTE**: `regen_fixtures` has been renamed to `regen_snapshots`.
- **NOTE**: Set `DIET_PYTHON_INTEGRATION_ONLY=1` to only transform integration test modules (skip transforming all imports).
- To inspect the transformed output of some code, run `cargo run --bin diet-python file_with_code.py`, which prints output to stdout.
- *MUST FOLLOW* when fixing a bug that fails a cpython test case *always* add a minimal reproducing integration test to reproduce it first.
- CPython source for tests lives in `../cpython` relative to this repo root (scripts expect the `python` binary there).
- **NOTE**: Default test protocol for `./scripts/pytest_cpython.sh tests/` is:
  - run with `DIET_PYTHON_INTEGRATION_ONLY=1` (transform only the integration module)
  - then run without it (transform all imports)
 
