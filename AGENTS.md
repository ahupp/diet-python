# AGENTS
- **MUST FOLLOW**: Format transform test sources as a single-line string when the source itself fits on one line. Otherwise, use a raw string literal like below.  Note that the
   first line of the statement is left-justified, and starts the line after the quotation mark.
   py_stmt!(r#"
def foo():
   return {expr:expr}
"#)

- **MUST FOLLOW**: Never add new variants to the minimal AST unless explicitly asked.
- **MUST FOLLOW**: Always run `cargo test` and `pytest` before submitting changes.
- **MUST FOLLOW**: Always preserve behavior in the transformed code, particularly evaluation order.
- **MUST FOLLOW**: For each integration test, add a desugaring test in a text file with the expected inputs and outputs.
- **NOTE**: Prefer adding behavior at transform time rather than runtime in `__dp__.py` whenever possible.
- **MUST FOLLOW**: If a fixture error occurs, regenerate all fixtures by running `cargo run --bin regen_fixtures` with no file arguments.
- **NOTE**: Use `cargo run --bin regen_fixtures` to regenerate fixtures instead of manual edits.
- Run tests with "cargo test", and "./scripts/pytest_cpython.sh tests/"
- To inspect the transformed output of some code, run `cargo run file_with_code.py`, which prints output to stdout.
 - *MUST FOLLOW* when fixing a bug that fails a cpython test case *always* add a minimal reproducing integration test to reproduce it first.
 
