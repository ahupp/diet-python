# AGENTS
- **MUST FOLLOW**: Never add new variants to the minimal AST unless explicitly asked.
- **MUST FOLLOW**: Always run `cargo test` and `./scripts/pytest_cpython.sh tests/` before submitting changes.
- **MUST FOLLOW**: Always preserve behavior in the transformed code, particularly evaluation order.
- **NOTE**: Prefer adding behavior at transform time rather than runtime in `__dp__.py` whenever possible.
- **MUST FOLLOW**: If a fixture error occurs, regenerate all fixtures by running `cargo run --bin regen_fixtures` with no file arguments.
- **NOTE**: Use `cargo run --bin regen_fixtures` to regenerate fixtures instead of manual edits.
- To inspect the transformed output of some code, run `cargo run --bin diet-python file_with_code.py`, which prints output to stdout.
- *MUST FOLLOW* when fixing a bug that fails a cpython test case *always* add a minimal reproducing integration test to reproduce it first.
- CPython source for tests lives in `../cpython` relative to this repo root (scripts expect the `python` binary there).
 
