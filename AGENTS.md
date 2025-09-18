# AGENTS
- **MUST FOLLOW**: For transform tests, compare the transformed code against the expected source using `assert_transform_eq`.
- **MUST FOLLOW**: Format transform test sources as a single-line string when the source itself fits on one line. Otherwise, use a raw string literal that begins with a newline so the code starts on the second line, for example:
  `let input = r#"\ndef foo():\n    return 1\n"#;`.
- **MUST FOLLOW**: Use multi-line strings for Python templates and other test examples, beginning with a newline so the code starts on the second line, unless a single-line string is explicitly required by the previous rule.
- **MUST FOLLOW**: Never add new variants to the minimal AST unless explicitly asked.
- **MUST FOLLOW**: Always run `cargo test` and `pytest` before submitting changes.
- **NOTE**: Transform tests go in a file named `test_module_name.txt`, containing zero or more blocks of the form:

  ```
  $ test name
  Input module
  =
  Output module
  ```

- **MUST FOLLOW**: Ensure new transform modules include the test execution macro in their test block.
