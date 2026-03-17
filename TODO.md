# Wishlist, DO NOT REMOVE

 * explicit, semantic meaning on passes (e.g, generators to yield from, pass N)
 * remove ruff StmtBody, use stock ruff
 * Use ruff scope analysis
 * simplify function-representation types through lowering; see [plans/FunctionTypes.md](plans/FunctionTypes.md)
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
    - As the ownership becomes explicit, split `await_lower.rs` apart so each helper lives in the pass module that actually owns that await-lowering stage instead of keeping one catch-all await helper module.
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
- Remove BB-lowering paths, and other unexpected late-stage dependencies, that pull Ruff `Stmt` / `Expr` back in after the semantic BlockPy boundary.
  - Planning note:
    - The desired end state is for BB lowering to analyze and normalize BlockPy directly instead of round-tripping through Ruff AST `Stmt` forms or depending on earlier Ruff `Expr` helpers unexpectedly late in the pipeline.
    - A good first pass is to audit all Ruff `Stmt` / `Expr` imports and call sites, confirm which ones are still expected at each lowering stage, and merge that inventory with the concrete BB-lowering round-trip cleanup.
    - This likely means replacing helper code that reconstructs `Stmt`/`StmtBody` for load-name, exception, or normalization analysis with BlockPy-native analysis utilities, while also tightening any remaining late-stage Ruff-expression dependencies to only the intended boundaries.
    - Keep the dataflow explicit so the BlockPy -> BB boundary no longer reintroduces earlier AST representations.
- Collapse the repeated Ruff/Semantic/Core BlockPy alias families into one stage-oriented representation, ideally via associated types on a stage trait or wrapper type.
  - Planning note:
    - Rust type aliases cannot themselves own associated types, so this likely needs either a `BlockPyStage` trait with associated types or stage wrapper newtypes rather than trying to hang associated types directly off `BlockPyModule`.
    - The goal is to stop spelling parallel alias lists for `Module`, `CallableDef`, `Block`, `Stmt`, `Term`, `Assign`, `If`, `Raise`, and related helpers, while still making the stage (`Ruff`, semantic, core) explicit.
    - This cleanup is now simpler because semantic BlockPy already carries Ruff `Expr` directly, so one whole stage-specific expression wrapper is gone from the matrix.
- Lift await and generator reduction into explicit top-level transform steps in `driver.rs`.
  - Planning note:
    - The desired end state is for `rewrite_module` to show the semantic BlockPy with raw `await` / generator forms, then explicit await lowering, then explicit generator reduction as separate visible steps.
    - This likely requires splitting the current hidden lowering helpers so await removal and generator lowering are typed bundle-to-bundle passes instead of internal side effects.
    - Keep the stage boundaries explicit in the driver so the top-level pipeline shows where those representations change.
- Use `ENTRY_BLOCK_LABEL` wherever a type models the exported callable entry, instead of leaking pre-normalization internal block labels.
  - Planning note:
    - Internal semantic/generator lowering still needs distinct local labels for resume, dispatch, factory, and relabelled CFG blocks.
    - But exported callable metadata and any AST-side preview of the callable should use `ENTRY_BLOCK_LABEL`, so the public entry invariant matches the later lowered BlockPy and BB stages.
    - A good first pass is to audit any preview, instantiation, or render-time entry-label fields and separate them from internal prepared-function labels explicitly.
- Evaluate the remaining BB-related types to see which ones can fold into the BlockPy/CFG generics.
  - Planning note:
    - `BbExpr` is already gone, so the next candidates are the BB-only wrappers and metadata types such as `BbFunction`, `BbBlock`, `BbTerm`, `BbOp`, and the closure-layout / function-kind families.
    - A good first pass is to separate “truly backend-specific” concerns from generic CFG/block/container structure, and check whether those backend-specific pieces can become generic parameters on the existing BlockPy/Cfg chassis instead of separate top-level BB concepts.
    - The outer `LoweredBlockPyFunction` / `BbFunction` merge is mostly done now: both stages share the generic `LoweredFunction<C, X>` shell plus `BoundCallable<C>`, and the remaining question is how much of the stage-specific metadata can be further shared.
    - The current likely split is that `bb_kind` / runtime closure metadata are shared lowered-runtime concerns, while `block_params` / `exception_edges` remain BlockPy-to-BB bridge side tables.
- Move refcount management out of `soac-eval` and into a new explicit pass in `rewrite_module`.
  - Planning note:
    - The current JIT path in `soac-eval` still owns a large amount of `incref` / `decref` insertion and runtime helper wiring (`dp_jit_incref`, `dp_jit_decref`), which makes ownership of reference semantics backend-local instead of pipeline-visible.
    - The desired end state is for refcount ownership to become an explicit lowered-module pass in `rewrite_module`, so later backends consume already-refcount-annotated IR instead of each backend re-deriving those rules.
    - A good first pass is to identify the minimal IR annotation or explicit stmt/term forms needed for retain/release edges, then move the current JIT-only reference-management decisions behind one driver-visible transform boundary.
- Fold `linearize_structured_ifs` into `lower_blockpy_blocks_to_bb_blocks`.
  - Planning note:
    - `lower_core_blockpy_function_to_bb_function` would read more clearly if it were mostly a direct lowered-function copy with one transform on the `blocks` field, instead of separately unpacking `linearize_structured_ifs(...)` first.
    - A good refactor is to make `lower_blockpy_blocks_to_bb_blocks` own the structured-if linearization plus BB block conversion, so the outer function becomes a straightforward metadata copy from the final core lowered function into the BB lowered function.
- Move `codegen_trace` to be a generic transform over `CfgModule`.
  - Planning note:
    - The current ownership under `blockpy_to_bb` suggests BB-specific trace injection, but the transform shape is really a CFG/module rewrite that should be expressible over generic `CfgModule` structure.
    - A good first pass is to separate BB-specific trace expression construction from the module/block traversal itself, then generalize the traversal layer so later stages can reuse the same trace-instrumentation transform over other `CfgModule` payloads.
- Review all visibility annotations and make them as restrictive as possible, moving helpers into the narrowest owning module when they are only consumed there.
  - Planning note:
    - The desired end state is that non-local visibility exists only for real cross-module boundaries, not as a convenience for call sites that could instead live beside their only consumers.
    - A good first pass is to audit `pub`, `pub(crate)`, and cross-module free functions, then inline or relocate single-consumer helpers before tightening the remaining visibility annotations.
    - Keep the resulting ownership aligned with the codebase-size goal: each concept should live in one place, and each place should expose only the smallest surface needed by later passes.
- Revisit `ruff_to_blockpy/expr_lowering/recursive.rs` and see whether the recursive expression lowering can be expressed as a `Transformer` over `Expr`.
  - Planning note:
    - The current file is a hand-written recursive traversal even though the repo rule is to prefer `Transformer`-based AST walks.
    - The key question is whether the setup-emitting behavior for boolop / compare / if-expr / named-expr / await / yield shapes can be preserved while letting a `Transformer` own the generic recursive descent.
    - A good first pass is to separate “plain recursive descent over child `Expr` nodes” from the setup-emitting special cases, then check if the former can move behind a reusable `Transformer` implementation.

## Completed

- Move completed TODO entries here and include a short description of the work done.
- Sequential string literal merge:
  - `lower_surrogate_string_literals` now first merges Ruff's implicitly concatenated string and bytes literal expressions into single logical literal nodes.
  - Surrogate decoding still runs after that normalization step, so later phases no longer need to reason about multi-part ordinary literal expressions.
- PassTracker explicit-dataflow shape:
  - `PassTracker::add_pass` is now `#[must_use]`, records per-pass elapsed time, and the CLI timing report includes ordered `pass_timings`.
  - The driver now tracks the real lowered semantic/core BlockPy bundles at the `add_pass(..., || { ... })` boundaries instead of eagerly projecting render-only `BlockPyModule` values.
  - Projection with `project_lowered_module_callable_defs` now happens at consumption sites like tests, snapshots, and the web inspector.
- String-template simplify-pass integration:
  - The standalone `lower_string_templates_in_lowered_blockpy_module_bundle` driver step is gone.
  - Semantic BlockPy now keeps raw f-strings/t-strings, and the main semantic-BlockPy -> core-BlockPy expr simplifier lowers them alongside the other core expression reductions.
  - The source-sensitive literal work remains earlier in `lower_surrogate_string_literals`, so the late string-template lowering stays context-free.
- Replace semantic `BlockPyExpr` with Ruff `Expr`:
  - Semantic BlockPy now carries Ruff `Expr` directly, so the semantic stage is expressed by the surrounding BlockPy module/callable/block types instead of a near-identity expression wrapper.
  - The wrapper enum and its conversion helpers are gone; `CoreBlockPyExpr` remains the real reduced expression boundary.
- Replace `BbExpr` with the final core BlockPy expression type:
  - BB IR, the JIT planner, and related tests/rendering code now use `CoreBlockPyExprWithoutAwaitOrYield` directly instead of a separate `BbExpr` wrapper/alias.
  - The remaining raw-`Expr` boundary normalization moved onto `CoreBlockPyExprWithoutAwaitOrYield::from_expr`, so BB-specific helper lowering no longer needs its own expression concept.
  - The expression layer no longer forks at the BB boundary, and the follow-up cleanup is now focused on the remaining BB-only function/block/container types.
- Merge `LoweredBlockPyFunction` and `BbFunction`:
  - Both stages now share the generic `LoweredFunction<C, X>` chassis and `BoundCallable<C>` in `lowered_ir.rs`, instead of maintaining separate outer wrapper concepts.
  - The BB side is now just an alias over that shared shell, and the remaining follow-up is metadata factoring rather than wrapper-shape unification.

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
