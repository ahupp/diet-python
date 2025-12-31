# AGENTS
- **MUST FOLLOW**: Format transform test sources as a single-line string when the source itself fits on one line. Otherwise, use a raw string literal like below.  Note that the
   first line of the statement is left-justified, and starts the line after the quotation mark.
   py_stmt!(r#"
def foo():
   return {expr:expr}
"#)

- **MUST FOLLOW**: Never add new variants to the minimal AST unless explicitly asked.
- **MUST FOLLOW**: Always run `cargo test` and `pytest` before submitting changes.
- **NOTE**: Transform tests go in a file named `test_module_name.txt`, containing zero or more blocks of the form:

  ```
  $ test name
  Input module
  =
  Output module
  ```
  Always prefer to update the expected output rather than the transform in the case of a mismatch, unless there's clearly a bug in the transform. 

- **MUST FOLLOW**: Ensure new transform modules include the test execution macro in their test block.
- **MUST FOLLOW**: For each integration test, add a desugaring test in a text file with the expected inputs and outputs.
- **NOTE**: Prefer adding behavior at transform time rather than runtime in `__dp__.py` whenever possible.
- Run tests with "cargo test", and "uvx pytest tests/"
