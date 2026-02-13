
## Follow-up: weakref callback during shutdown (eval/BB mode)

- Symptom:
  - Sharded CPython run reports at process shutdown:
    - `Exception ignored while calling weakref callback <function _removeHandlerRef ...>`
    - Trace enters `__dp__.py`:
      - `entry` at `__dp__.py:1993`
      - `run_bb` at `__dp__.py:778`
      - `AttributeError: 'NoneType' object has no attribute 'take_arg1'`
- Repro context:
  - Seen in eval mode on fast shard runs (BB lowering enabled by default), e.g.:
    - `logs/cpython_eval_test_set_part01_after_targeted_for_fix.log`
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
  - Keep behavior unchanged for normal execution/eval order; this is a teardown robustness fix.
- Suggested validation:
  - Add a regression that simulates late callback execution after clobbering module globals (or equivalent teardown simulation) and verifies no `AttributeError` from `__dp__` lookups.
  - Re-run shard `test_sets/cpython_fast_tests_part_01.txt` and confirm warning disappears.
