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
- Remove the “start label” concept and always make the first block the callable entry block.
  - Planning note:
    - The desired end state is that callable entry is represented structurally by block order, with block `0` / the first block as the entry block, instead of carrying a separate exported start-label concept.
    - A good first pass is to audit every place that stores, normalizes, renders, or exports a start/entry label and separate internal relabeling concerns from public callable entry semantics.
    - Then make CFG/BlockPy/BB construction normalize blocks so the entry block is first, and delete the extra start-label plumbing from previews, rendering, and lowered/export metadata.
  - Allow fallback to bytecode for arbitrary functions, use this for __annotate__

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
- Evaluate the remaining BB-related types to see which ones can fold into the BlockPy/CFG generics.
- Collapse the repeated Ruff/Semantic/Core BlockPy alias families into one stage-oriented representation, ideally via associated types on a stage trait or wrapper type.
- Remove the fallback await-lowering path so all awaits use one explicit pass, and make that pass appear as a top-level step in `rewrite_module`.
- Add an evaluation-order-explicit pass that hoists composite subexpressions into temps while preserving left-to-right evaluation, e.g. `a = foo(b(), c)` -> `tmp = b(); a = foo(tmp, c)`.


