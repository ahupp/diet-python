
## Codex TODO Intake

- Reserved for user requests that start with `TODO`.
- Add one entry per request and include any plan or relevant response summary with it.


- there are many places where we switch behavior based on the names of things, ex:
    * _dp_class_ns_
    * __dp_decode_literal_bytes
    * should_strip_nonlocal_for_bb
    * _dp_self
    * _dp_cell_
    * _dp_try_exc_
    * _dp_classcell

- Avoid collisions between generated temp/block names and user-written names.
  - Planning note:
    - `NameGen` should be reusable per `BlockPyFunction`, instead of each pass inventing fresh local counters, so later transforms stay in one generated-name namespace.
    - The current simplification is to stop inspecting locals for reservations and rely on the generated namespace shape; this keeps the pipeline simpler but does not prove collision-freedom.
    - The likely real fix is either a non-string temp/id representation carried through the IR, or one late legalization/materialization pass that checks concrete Python names once.

- Everything about annotation_export.rs needs revisiting.
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
- Revisit `ruff_to_blockpy/expr_lowering/recursive.rs` and see whether the recursive expression lowering can be expressed as a `Transformer` over `Expr`.
  - Planning note:
    - The current file is a hand-written recursive traversal even though the repo rule is to prefer `Transformer`-based AST walks.
    - The key question is whether the setup-emitting behavior for boolop / compare / if-expr / named-expr / await / yield shapes can be preserved while letting a `Transformer` own the generic recursive descent.
    - A good first pass is to separate “plain recursive descent over child `Expr` nodes” from the setup-emitting special cases, then check if the former can move behind a reusable `Transformer` implementation.
  - Allow fallback to bytecode for arbitrary functions, use this for __annotate__
- Figure out how to make classcell work with the rest of name binding.
  - Planning note:
    - `__class__` / classcell handling is still outside the normal semantic binding pipeline, with dedicated rewrites in the class method rewrite path instead of flowing through `BlockPyCallableSemanticInfo` and `name_binding`.
    - The likely end state is to model `__class__` as a synthetic cell capture for methods that need it, keep `__classcell__` as the class-creation protocol surface, and let `name_binding` lower `__class__` loads/stores/deletes through the same cell machinery as other captures.
    - A good first pass is to identify the minimal semantic facts needed for “method needs class cell”, then thread a synthetic binding for `__class__` through callable semantic info before shrinking the remaining special cases.
- Should there be a `py_stmt` -> `BlockPyCfgFragment` path to ease building generators?
  - Planning note:
    - `blockpy_generators` still hand-constructs a large amount of BlockPy stmt/term/control-flow scaffolding, which makes generator lowering harder to read and keeps a lot of structural knowledge local to that pass.
    - A helper path from `py_stmt!`-style snippets into `BlockPyCfgFragment` could make generator construction less manual, especially for small setup/cleanup fragments and repeated control-flow shapes.
    - The main design question is whether that path would preserve the current guarantees around evaluation order, hidden temps, and explicit block structure, or whether it would just hide logic that should instead be expressed by more explicit BlockPy builders.
- Should we linearize in the BlockPy pass so the whole block structure is uniform?
  - Planning note:
    - BlockPy still carries structured non-terminal `If` for a long stretch of the pipeline, and only later linearizes it into true block terminators.
    - The likely simplification is to move `linearize_structured_ifs` earlier so downstream passes only need to reason about one CFG/block shape instead of both structured fragments and linearized blocks.
    - A good first pass is to identify which post-BlockPy passes still recurse through structured `BlockPyStmt::If`, then check whether running linearization immediately after semantic/core BlockPy lowering would shrink those passes instead of complicating exception-edge or block-param handling.
- Handle integer literals larger than can fit in an `i64`.
  - Planning note:
    - The current direct-simple JIT literal planning in `soac-eval/src/jit/planning.rs` only lowers integer literals that fit in `i64`, so larger Python ints fall out of that fast path.
    - A good first pass is to decide whether large ints should be materialized through a general Python-object literal helper at planning/codegen time, or whether they should be excluded from the direct-simple subset in a more explicit way.
## Completed

- Move completed TODO entries here and include a short description of the work done.
- Ensure `blockpy_expr_simplify` panics if it receives an expression shape that should already have been removed by `rewrite_ast_to_lowered_blockpy_module_plan`.
  - `blockpy_expr_simplify` now validates incoming semantic `Expr` trees with a `Transformer` before any core lowering work.
  - Helper-scoped expression families that should already have been rewritten away there, namely lambdas, generator expressions, and comprehensions, now panic immediately with a boundary-specific invariant message.
  - Added a focused regression that proves a leaked nested lambda trips that simplify-pass boundary.
- Eliminate the temporary Ruff semantic pass split:
  - `rewrite_ast_to_lowered_blockpy_module_plan_with_module` now emits lowered semantic blocks directly, threads exception edges recursively during semantic lowering, and no longer needs a metadata-free intermediate Ruff pass shape.
  - The remaining Ruff-backed semantic pass marker was then renamed back to `RuffBlockPyPass`, so there is again just one Ruff semantic BlockPy stage instead of a `LoweredRuffBlockPyPass` / `RuffBlockPyPass` split.
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
- Implement a BlockPyModuleVisitor, analagous to BlockPyModuleMap.  This will visit everything in order, taking by reference not value.  It should have a &mut self reciever.  Then move all the summarize_ stuff in basic_block/mod.rs to it's own module, and use a BlockPyModuleVisitor to do that generically.
- I don't think flatten_stmt_boxes and flatten_stmt do anything anymore, remove
- merge bound_names into ast_symbol_analysis
- There is pretty-print logic in bb_ir.rs, web_inspector.rs, and block_py/pretty.rs. \ Determine if all those can be merged into a single implementation, possibly with BlockPyModuleVisitor.
- move bb_ir into blockpy_to_bb/mod.rs
- move "block_py" to be a top-level module.
- rename the "basic_block" module to "passes"
- Move `codegen_trace` to be a generic transform over `CfgModule`.
- Remove the “start label” concept and always make the first block the callable entry block.
- Determine if codegen_trace.rs and cfg_trace.rs are doing similar things, and merge if so.
- Simplify should remove literals for true/false/none/ellipsis, replacing them with their _dp_ versions, remove that from codegen_normalize.  Remove those from the expr ast.
