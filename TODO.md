
## Follow-up: weakref callback during shutdown (BB mode)

- Symptom:
  - Sharded CPython run reports at process shutdown:
    - `Exception ignored while calling weakref callback <function _removeHandlerRef ...>`
    - Trace enters `__dp__.py`:
      - `entry` at `__dp__.py:1993`
      - `run_bb` at `__dp__.py:778`
      - `AttributeError: 'NoneType' object has no attribute 'take_arg1'`
- Repro context:
  - Seen in transformed fast shard runs (BB lowering enabled by default), e.g.:
    - `logs/cpython_transform_test_set_part01_after_targeted_for_fix.log`
  - The shard still exits `0`, so this is currently a shutdown warning, not a hard failure.
- Working hypothesis:
  - BB block functions still resolve `__dp__` from module globals at call time.
  - During interpreter/module teardown, module globals are cleared to `None`.
  - Late weakref callbacks (for example from `logging._removeHandlerRef`) can run after this, so transformed callback code executes with `__dp__ is None`.
  - That makes emitted block code like `__dp__.take_arg1(...)` fail.
- Why this is important:
  - Indicates transformed functions are not teardown-safe when invoked late in shutdown.
  - Could become noisy across stdlib code paths that rely on weakref finalizers/callbacks.
- Suggested fix direction:
  - Make BB block call paths independent of module-global `__dp__` at runtime:
    - Prefer capturing `__dp__` as a default/closure on emitted block functions (not only wrappers).
    - Or otherwise ensure block runtime helpers used by blocks are bound/captured and not global lookups.
  - Keep behavior unchanged for normal execution order; this is a teardown robustness fix.
- Suggested validation:
  - Add a regression that simulates late callback execution after clobbering module globals (or equivalent teardown simulation) and verifies no `AttributeError` from `__dp__` lookups.
  - Re-run shard `test_sets/cpython_fast_tests_part_01.txt` and confirm warning disappears.

## Follow-up: traceback/source lookup mismatch for transformed modules

- Symptom:
  - pytest can hit `INTERNALERROR` while rendering failures:
    - `_pytest/_code/source.py:get_statement_startend2`
    - `IndexError: list index out of range`
- Repro context:
  - JIT-enabled runs with transformed integration packages where `__init__.py` is empty on disk.
  - The transformed execution raises at a line in `__init__.py` (for example line 3), but the file on disk has zero lines.
- Working hypothesis:
  - `co_filename` points to original source path while executed transformed code has different line layout.
  - pytest reads AST/source from disk for that filename and crashes when traceback line exceeds available statements.
- Suggested fix direction:
  - Keep traceback/source mapping coherent for transformed code:
    - either preserve a synchronized source cache (for `linecache`) keyed by `co_filename`,
    - or use a dedicated synthetic filename/path for transformed code with matching stored source text.
  - Ensure transformed module line numbers map to the source that traceback tooling loads.
- Suggested validation:
  - Add a regression that imports a transformed module with empty source and forces an exception from transformed code.
  - Verify `pytest` default traceback mode reports normal failure (no `INTERNALERROR`), and source lines render correctly.
