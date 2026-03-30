
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
- Merge simplify into the BlockPy pass and run it bottom-up so it is one-shot.
  - Planning note:
    - `blockpy_expr_simplify` is currently a separate pass boundary after semantic BlockPy construction, even though conceptually it is just finishing the lowering of expressions into core BlockPy form.
    - The likely simplification is to fold that work into the BlockPy lowering pass itself and run expression reduction bottom-up, so expressions only cross one lowering seam instead of first building semantic BlockPy exprs and then revisiting them.
    - A good first pass is to list which invariants `blockpy_expr_simplify` currently enforces for later passes, then check whether those can be guaranteed directly during semantic BlockPy construction without losing the current clear boundary for invalid leaked expr shapes.
- Add a pass for specific storage decisions, closure slot offsets, and stack offsets.
  - Planning note:
    - Several later stages still implicitly decide concrete storage/layout details such as which values live in cells, the ordering of closure slots, and stack/local slot numbering.
    - A dedicated pass for those decisions would make the backend-facing layout explicit, instead of spreading that knowledge across generator lowering, closure construction, and codegen-adjacent logic.
    - A good first pass is to identify which existing decisions are semantic versus purely physical layout, then choose one IR boundary where storage class, closure slot index, and stack/local offsets become fixed and immutable.
- Compute `ClosureLayout` in `name_binding`, and keep all closure data semantic before that.
  - Planning note:
    - `ClosureLayout` currently crosses the semantic/storage boundary too early even though it mixes runtime storage layout with logical capture information.
    - The desired end state is for pre-`name_binding` passes to carry only semantic closure facts, and for `name_binding` to materialize the first real `ClosureLayout` with a clear distinction between logical names and storage slots.
    - Later passes that add new freevars, cellvars, or runtime closure cells should update `ClosureLayout` through one explicit API or validation hook, so layout drift cannot be introduced silently.
- Remove the `_dp_resume` closure-layout refresh special case by making later closure-layout mutations explicit.
  - Planning note:
    - The current unconditional closure-layout refresh had to grow a special case for synthetic `_dp_resume` callables because their runtime closure layout is no longer derivable from ordinary semantic capture facts.
    - The desired end state is for refresh/recompute logic to stop guessing about post-`name_binding` runtime layouts, and for generator/resume lowering to own its closure-layout mutations through explicit APIs or phase-local construction.
    - A good first pass is to identify every later pass that mutates closure storage shape, then make those updates visible as explicit `ClosureLayout` edits or validations instead of patching over them with name-based exclusions.
- Audit remaining `diet-python` naming and update user-facing/project naming to `soac` where appropriate.
  - Planning note:
    - The crate rename to `soac-blockpy` removed one major old name seam, but there are still likely package, binary, doc, log, and runtime-visible references to `diet-python`.
    - The goal is to identify which of those are intentional compatibility surfaces and which are just stale internal/project naming.
    - A good first pass is to inventory repo-wide `diet-python` mentions, group them into code/runtime/docs/tooling buckets, and then rename the non-compatibility cases first.
- Story for constants (`None`, strings, etc.).
  - Planning note:
    - The pipeline still has multiple places that decide how constants are represented, including literal expr forms, `_dp_` builtins, and backend/runtime materialization paths.
    - The desired end state is to have one clear story for when constants remain abstract IR literals versus when they become fixed runtime objects or named runtime helpers.
    - A good first pass is to inventory the current handling of `None`/`True`/`False`/ellipsis, strings/bytes, tuples of constants, and large literals, then choose one pass boundary where constant representation becomes final for all backends.
- Handle integer literals larger than can fit in an `i64`.
  - Planning note:
    - The current direct-simple JIT literal planning in `soac-eval/src/jit/planning.rs` only lowers integer literals that fit in `i64`, so larger Python ints fall out of that fast path.
    - A good first pass is to decide whether large ints should be materialized through a general Python-object literal helper at planning/codegen time, or whether they should be excluded from the direct-simple subset in a more explicit way.
- Give intrinsics typed expr builders instead of raw `Vec` arg construction.
  - Planning note:
    - Call sites like `core_positional_intrinsic_expr_with_meta(&MAKE_CELL_INTRINSIC, ..., vec![init])` still encode intrinsic arity and argument ordering implicitly in the call site.
    - The desired end state is for each intrinsic type itself, e.g. `MakeCellIntrinsic`, to expose typed constructors like `expr_with_range(range, arg0)` and `expr_without_range(arg0)` so arity mismatches become type errors instead of runtime/assertion bugs.
    - A good first pass is to define a trait implemented by the intrinsic singleton types, add the fixed-arity constructors for a few common intrinsics, and then remove the matching raw-`Vec` helper calls so the old untyped path does not remain as a compatibility layer.
- Make a plan for accurate source-region tracking on emitted instructions.
  - Planning note:
    - Many lowering paths still stamp emitted exprs/stmts/terms with `default()` node/range metadata, so provenance becomes inconsistent once code is synthesized across multiple transform boundaries.
    - The desired end state is to have one explicit story for where each emitted instruction’s source range comes from: original source span, enclosing source span, or a clearly-marked synthetic span.
    - A good first pass is to inventory the current `compat_*`, `Default::default()`, and synthetic-meta call sites, group them by kind of emission, and choose one boundary where source provenance becomes mandatory and validated for every emitted instruction.
- Clean up the conversions and related glue in `block_py/mod.rs`.
  - Planning note:
    - `block_py/mod.rs` still carries a large amount of conversion glue between neighboring IR/pass shapes, including expression/statement/container conversions that make the file harder to navigate than the core BlockPy model itself.
    - The desired end state is for `block_py/mod.rs` to mainly define the shared IR types and only the most fundamental traits/helpers, with conversion-heavy code moved closer to the pass boundary that owns it or folded into a smaller explicit conversion module.
    - A good first pass is to inventory the `From`/`Into`/helper conversion blocks there, group them by source/target boundary, and then peel off one coherent cluster at a time so the file stops being the catch-all for unrelated stage transitions.
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

# QA

- Add question/answer entries here when the user asks a question and the response does not involve code changes.
- Include a date/timestamp with each entry.
- 2026-03-29 21:35:09 PDT
  - Question: What depends on the conversion traits like `impl From<CoreBlockPyExprWithAwaitAndYield> for Expr`?
  - Answer: There are two real dependency buckets. First, the `Into<Expr>` / `From<...> for Expr` conversions are used by generic BlockPy infrastructure that reifies a pass-local expression back into Ruff AST for inspection or rendering. The main sites are the generic expr traits in `BlockPyExprLike`, `MapExpr`, and `TryMapExpr`, at `/home/adam/project/diet-python/soac-blockpy/src/block_py/mod.rs:111`; the pretty-printer, which stringifies expressions via `ruff_ast_to_string(&expr.clone().into())`, at `render_expr`, in `/home/adam/project/diet-python/soac-blockpy/src/block_py/pretty/mod.rs:480`; the generic dataflow/name-collection helpers, which inspect names by converting lowered exprs back to AST, in `/home/adam/project/diet-python/soac-blockpy/src/block_py/dataflow/mod.rs:269`; and the capture-name scan in `collect_cell_ref_logical_names_in_stmt` / `collect_cell_ref_logical_names_in_term`, at `/home/adam/project/diet-python/soac-blockpy/src/passes/ruff_to_blockpy/mod.rs:259`, which walks lowered expressions by first converting them back to Ruff `Expr`. Second, the `TryFrom` conversions between pass-local expr flavors, especially `CoreBlockPyExprWithAwaitAndYield -> CoreBlockPyExprWithYield` and `CoreBlockPyExprWithYield -> CoreBlockPyExpr`, are on the real lowering path: `lower_awaits_in_core_blockpy_module`, in `/home/adam/project/diet-python/soac-blockpy/src/passes/core_await_lower/mod.rs:11`, depends on the default `BlockPyModuleMap` mapping between those pass types, and generator lowering uses `try_into()` heavily to assert that yield-like sites were split before converting to yield-free core expressions, in `lower_stmt_no_yield`, `lower_term_no_yield`, and `yield_value_expr`, at `/home/adam/project/diet-python/soac-blockpy/src/passes/blockpy_generators/mod.rs:576`. So the exact `From<...> for Expr` impls are mostly supporting generic AST reconstruction, printing, and analysis, while the `TryFrom` family is part of the actual await/yield pass pipeline. They are not primarily a JIT/runtime dependency.
- 2026-03-29 21:13:15 PDT
  - Question: In `BlockPyModuleMap`, do `map_delete` and `map_assign` ever get implemented in some trait impl?
  - Answer: Not directly. In `BlockPyModuleMap`, at `/home/adam/project/diet-python/soac-blockpy/src/block_py/convert.rs:140`, `map_assign(...)` has a default implementation, and at `/home/adam/project/diet-python/soac-blockpy/src/block_py/convert.rs:150`, `map_delete(...)` has a default implementation as well. Looking at the actual `impl BlockPyModuleMap<...>` sites in `/home/adam/project/diet-python/soac-blockpy/src/passes/name_binding.rs:904`, `/home/adam/project/diet-python/soac-blockpy/src/passes/name_binding.rs:1672`, `/home/adam/project/diet-python/soac-blockpy/src/passes/core_await_lower/mod.rs:6`, and `/home/adam/project/diet-python/soac-blockpy/src/passes/blockpy_to_bb/strings/mod.rs:15`, none of them override the trait `map_delete(...)`, and none of them override the trait-signature `map_assign(...)` either. The one special case is `NameBindingMapper`: in `impl BlockPyModuleMap<CoreBlockPyPass, CoreBlockPyPass> for NameBindingMapper`, at `/home/adam/project/diet-python/soac-blockpy/src/passes/name_binding.rs:904`, it overrides `map_stmt(...)`, and inside that override it handles `BlockPyStmt::Assign(assign)` by calling an inherent helper `NameBindingMapper::map_assign(...)`, at `/home/adam/project/diet-python/soac-blockpy/src/passes/name_binding.rs:1003`. So assignment behavior is customized there, but not by overriding the trait method itself. The direct answer is: `map_delete` always uses the trait default, and the trait `map_assign` also always uses the trait default; only `NameBindingMapper` bypasses that default path indirectly through its custom `map_stmt(...)`.
- 2026-03-29 12:59:48 PDT
  - Question: With `name_binding` becoming authoritative for storage locations, is the `ambient_storage_names()` filtering in `exc_dispatch_plan(...)` still necessary? Can `ambient_storage_names()` be removed entirely, and if not, what would be a better name?
  - Answer: The broad partitioning in `exc_dispatch_plan(...)`, at `/home/adam/project/diet-python/soac-eval/src/jit/planning.rs:36`, is still necessary because exception dispatch still has to decide which target params are carried as real runtime block params and which ones must be written into persistent storage before the jump. What now looks stale is specifically the `ambient_storage_names()` part of that decision. With `StorageLayout.stack_slots()` becoming the authoritative codegen storage inventory, the more direct rule is “write a slot only if the target param has a stack slot and is not a runtime block param.” The current `ambient_storage_names()` filter is only being used in production there; the other callers are test code in `/home/adam/project/diet-python/soac-pyo3/src/test.rs:88` and `/home/adam/project/diet-python/soac-pyo3/src/test.rs:102`. So production-wise, yes, it looks removable in favor of `stack_slots()`. If the helper survives for tests or other debug surfaces, `ambient_storage_names()` is too vague; the clearest replacement would be `inherited_capture_storage_names()`, because what it actually returns in `/home/adam/project/diet-python/soac-blockpy/src/block_py/mod.rs:281` is the storage names of `ClosureSlot`s whose `init` is `ClosureInit::InheritedCapture`, i.e. storage coming from outer-scope captures rather than locals or runtime-introduced cells.
- 2026-03-29 11:32:39 PDT
  - Question: How is `function_state_slot_names` defined/used ?
  - Answer: In `function_state_slot_names`, at `/home/adam/project/diet-python/soac-eval/src/jit/mod.rs:583`, it builds a deduplicated ordered list of names that need persistent stack-backed storage in the specialized JIT function. The list is assembled from closure-layout ambient storage names, closure-layout local cell storage names, ordinary function parameter names, and block parameter names. That list is consumed by `FunctionStateSlots::new`, at `/home/adam/project/diet-python/soac-eval/src/jit/mod.rs:519`, which allocates one Cranelift stack slot per name. Those slots are then used as the per-function state store: entry initialization writes defaults and incoming args into them, block entry writes runtime block params into them, name loads can read from them through `load_function_state_value`, and assignments/deletes update them through `replace_cloned_value`. So conceptually `function_state_slot_names` is the inventory of names that should survive across block boundaries in the specialized JIT path, as opposed to transient SSA locals stored only in `local_names` / `local_values`.
- 2026-03-29 12:23:00 PDT
  - Question: It seems like `name_binding` should have this responsibility. What needs to be added there so that `name_binding` is the source of truth for storage locations?
  - Answer: `name_binding` is already very close, but it stops one step short of becoming the backend-facing storage authority. In `collect_local_slot_locations`, at `/home/adam/project/diet-python/soac-blockpy/src/passes/name_binding.rs:1396`, it already computes a stable slot numbering for params, owned-cell storage names, block params, and remaining locals, and `locate_name`, at `/home/adam/project/diet-python/soac-blockpy/src/passes/name_binding.rs:1478`, writes those slot numbers into `NameLocation::Local { slot }`. The gap is that the specialized JIT still ignores those local slot numbers for ordinary loads/stores and instead reconstructs a parallel string-keyed inventory with `function_state_slot_names`, at `/home/adam/project/diet-python/soac-eval/src/jit/mod.rs:583`, then looks up slots by `name.id`. To make `name_binding` the source of truth, the lowered IR needs one explicit function-state layout attached after `name_binding`, for example a `FunctionStorageLayout` on `BlockPyFunction`, with an ordered slot table and stable numeric slot ids. Then `NameLocation::Local { slot }` should mean the final function-state slot index, `BlockParam` should carry or imply its storage slot directly instead of only a name/role, and entry parameter binding should map incoming params to those same slots without name lookups. `ClosureLayout` can remain the source of truth for closure-cell slot order, but any stack-backed aliases derived from it should also be materialized into that function-state layout in `name_binding`, not rediscovered later by the JIT from `storage_name` strings.
