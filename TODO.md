
## Codex TODO Intake

- Reserved for user requests that start with `TODO`.
- Add one entry per request and include any plan or relevant response summary with it.

- Move bb_ir.rs into blockpy_to_bb.  Also, BbModule seems to have it's own pretty-printing path; unify that with the other printing paths using the new BlockPyModuleMap trait.  Move web_inspector_support.rs to a "pretty.rs", could prob merge with that existing pretty.rs too.
- Determine if codegen_trace.rs and cfg_trace.rs are doing similar things.
- move all the summarize_ stuff in basic_block/mod.rs to it's own module, and use a BlockPyModuleMap to do that generically.
- there are many places where we switch behavior based on the names of things, searching for _dp_class_ns_, __dp_decode_literal_bytes, should_strip_nonlocal_for_bb
- Everything about annotation_export.rs needs revisiting.
- I don't think flatten_stmt_boxes and flatten_stmt do anything anymore, remove
- merge bound_names into ast_symbol_analysis

- Use Ruff for scope analysis and see if it can be computed once and preserved through transform layers.
  - Planning note:
    - The desired end state is to replace local repeated scope-analysis passes with Ruff’s scope analysis and carry that result through later transform phases instead of recomputing scope metadata.
    - This likely requires identifying the current pass boundaries that invalidate or rebuild scope information, then either preserving Ruff scope objects directly or translating them once into a stable internal form.
    - Keep the scope-analysis ownership explicit in the top-level pipeline so later passes consume preserved scope data rather than silently re-running analysis.
- Move refcount management out of `soac-eval` and into a new explicit pass in `rewrite_module`.
  - Planning note:
    - The current JIT path in `soac-eval` still owns a large amount of `incref` / `decref` insertion and runtime helper wiring (`dp_jit_incref`, `dp_jit_decref`), which makes ownership of reference semantics backend-local instead of pipeline-visible.
    - The desired end state is for refcount ownership to become an explicit lowered-module pass in `rewrite_module`, so later backends consume already-refcount-annotated IR instead of each backend re-deriving those rules.
    - A good first pass is to identify the minimal IR annotation or explicit stmt/term forms needed for retain/release edges, then move the current JIT-only reference-management decisions behind one driver-visible transform boundary.
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
- Revisit the split between `YieldLoweringModuleMap` and `YieldLoweringMap`.
  - Planning note:
    - The current split in `dp-transform/src/basic_block/blockpy_to_bb/mod.rs` exists because yield-lowering wants the per-function `qualname` in its panic message, and the default `BlockPyModuleMap` recursion only gives `map_expr` the expression value, not the enclosing function context.
    - `YieldLoweringModuleMap::map_module` currently constructs a fresh `YieldLoweringMap { qualname }` per callable and then uses the default recursive `map_fn`, so the duplication is only there to thread that function-local context.
    - Follow-up options:
      - drop the qualname-specific panic context and use one mapper with only `map_expr`
      - extend the generic mapper API with a function-context hook
      - or keep the split if per-function panic context is worth the extra type

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
- Remove local `StmtBody` usage and move back to upstream Ruff structures.

