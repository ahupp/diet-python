# Wishlist, DO NOT REMOVE

 * explicit, semantic meaning on passes (e.g, generators to yield from, pass N)
 * remove ruff StmtBody, use stock ruff
 * Use ruff scope analysis
 * Search for dead functions
 * try_lower_function_to_blockpy_bundle, wtf

## Codex TODO Intake

- Reserved for user requests that start with `TODO`.
- Add one entry per request and include any plan or relevant response summary with it.
- Add an evaluation-order-explicit pass that hoists composite subexpressions into temps while preserving left-to-right evaluation, e.g. `a = foo(b(), c)` -> `tmp = b(); a = foo(tmp, c)`.
  - Planning note:
    - The pass should make effect order explicit before the await/generator boundary so later phases only see atomic operands in control/runtime positions.
    - As an implied invariant, only names should be allowed as operands to `If`, `Return`, `Raise`, `Await`, `Yield`, and `YieldFrom`.
    - Call arguments are the motivating first example, but the pass should cover any composite expression whose evaluation order must be made explicit.
- Remove the fallback await-lowering path so all awaits use one explicit pass, and make that pass appear as a top-level step in `rewrite_module`.
  - Planning note:
    - Split the current coarse AST-to-lowered-BlockPy boundary so `rewrite_module` can show an explicit semantic-BlockPy-with-awaits -> semantic-BlockPy-without-awaits step.
    - Route all async functions through semantic BlockPy first, then run one bundle-level await-lowering pass instead of probing an AST fallback path.
    - Widen that pass until it handles every semantic position that can contain `await`, then delete the fallback route and legacy gating fields.
    - Keep the final design visible in `rewrite_module` as a real typed phase boundary instead of hiding it inside a lower-level helper.
- Fold `lower_string_templates_in_lowered_blockpy_module_bundle` into the main expr simplification pass instead of keeping it as a standalone driver step.
  - Planning note:
    - The desired end state is for late string-template lowering to be part of the same bundle-level simplify pass that produces core BlockPy.
    - Verified note:
      - Ruff already stores decoded string content in `StringLiteral.value`, and `ExprStringLiteral.value.to_str()` returns the concatenated decoded value for implicitly concatenated literals.
      - That means `Context` does not appear to be needed just to recover the ordinary Python string value.
      - Ruff parser utilities worth investigating here are `ruff_python_parser::string` for string-literal parsing/decoding and the lexer string handling in `ruff_python_parser::lexer`.
      - Adjacent string literals are already represented as `StringLiteralValue::concatenated(...)` during parsing in `ruff_python_parser::parser::expression`, so early literal merging may be more about normalizing existing Ruff concatenation nodes than inventing a new merge step from scratch.
    - The remaining `Context` dependency looks narrower: source-sensitive handling that still cares about the original literal spelling, especially surrogate-escape detection/decoding.
    - Move that source-sensitive non-raw-string work into `lower_surrogate_string_literals` so it happens earlier, while original source information is still available.
    - Ideally move adjacent string-literal merging there too, so sequential literals become one logical string before the later semantic-BlockPy string-template phase.
    - After that split, the remaining late string-template lowering should be context-free enough to fold into the main lowered-BlockPy expr simplifier as part of the semantic-BlockPy -> core-BlockPy reduction.
- Merge sequential string literals into a single logical string before later lowering phases.
  - Planning note:
    - Adjacent string literals should normalize to one logical string value before later expr/string lowering so the rest of the pipeline does not have to reason about multi-literal concatenation shapes.
    - This should preserve Python evaluation behavior and source-sensitive string handling, including any interaction with later f-string/t-string or surrogate-aware lowering.
    - The likely boundary is the early expr normalization pipeline, before the later semantic-BlockPy string-template handling.
- Remove local `StmtBody` usage and move back to upstream Ruff structures.
  - Planning note:
    - The desired end state is to stop depending on the local `StmtBody` wrapper and align the lowering pipeline back with upstream Ruff AST/container shapes.
    - This likely requires auditing every pass boundary that currently takes or returns `StmtBody`, then replacing those boundaries one by one with upstream Ruff forms instead of doing a single large delete.
    - Keep the migration explicit in the top-level pipeline so container-shape normalization is no longer hidden inside helper layers.
- Use Ruff for scope analysis and see if it can be computed once and preserved through transform layers.
  - Planning note:
    - The desired end state is to replace local repeated scope-analysis passes with Ruff’s scope analysis and carry that result through later transform phases instead of recomputing scope metadata.
    - This likely requires identifying the current pass boundaries that invalidate or rebuild scope information, then either preserving Ruff scope objects directly or translating them once into a stable internal form.
    - Keep the scope-analysis ownership explicit in the top-level pipeline so later passes consume preserved scope data rather than silently re-running analysis.
- Remove BB-lowering paths that convert BlockPy back into Ruff `Stmt` nodes just to do analysis.
  - Planning note:
    - The desired end state is for BB lowering to analyze and normalize BlockPy directly instead of round-tripping through Ruff AST `Stmt` forms.
    - This likely means replacing helper code that reconstructs `Stmt`/`StmtBody` for load-name, exception, or normalization analysis with BlockPy-native analysis utilities.
    - Keep the dataflow explicit so the BlockPy -> BB boundary no longer reintroduces earlier AST representations.

## Completed

- Move completed TODO entries here and include a short description of the work done.
- PassTracker explicit-dataflow shape:
  - `PassTracker::add_pass` is now `#[must_use]`, records per-pass elapsed time, and the CLI timing report includes ordered `pass_timings`.
  - The driver now tracks the real lowered semantic/core BlockPy bundles at the `add_pass(..., || { ... })` boundaries instead of eagerly projecting render-only `BlockPyModule` values.
  - Projection with `project_lowered_module_callable_defs` now happens at consumption sites like tests, snapshots, and the web inspector.

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

## Follow-up: JIT interception granularity via vectorcall

- Goal:
  - Use `PyFunction_SetVectorCall` to intercept only functions that should run through JIT.
  - Remove reliance on blanket interception of every function call path.
- Why this is important:
  - Reduces global behavior changes and narrows JIT integration surface.
  - Makes non-JIT functions continue on default CPython call path with less risk of regressions.
- Suggested implementation direction:
  - Register vectorcall override only for transformed/JIT-planned functions at function creation time.
  - Keep per-function metadata lookup in the vectorcall target to dispatch only when a JIT plan exists.
  - Preserve stock vectorcall for all other functions.
- Suggested validation:
  - Add regression coverage showing transformed planned functions enter JIT vectorcall path.
  - Add regression coverage showing unplanned/untransformed functions still use normal CPython vectorcall behavior.

## Follow-up: bytecode fallback for non-JITted functions

- Goal:
  - Ensure functions without a JIT plan execute through normal CPython bytecode evaluation.
- Why this is important:
  - Keeps unsupported or intentionally non-JITted shapes correct without forcing import-time failures.
  - Allows incremental JIT rollout while preserving behavior coverage.
- Suggested implementation direction:
  - At dispatch time, detect whether a function/code object has a compiled JIT plan.
  - If not present, immediately route to standard bytecode execution path for that function.
  - Keep this fallback explicit and per-function (no module-wide downgrade unless explicitly requested).
- Suggested validation:
  - Add tests where mixed modules contain both JITted and non-JITted functions and verify both paths execute correctly.
  - Add regression confirming unsupported JIT shapes run via bytecode fallback and preserve semantics.
