
## Codex TODO Intake

- Reserved for user requests that start with `TODO`.
- Add one entry per request and include any plan or relevant response summary with it.

## Replace *Expr::Name with LoadName operation

- Planning note:
  - Today `CoreBlockPyExprWithAwaitAndYield`, `CoreBlockPyExprWithYield`, `CoreBlockPyExpr<N>`, and `CodegenBlockPyExpr<N>` all still have a first-class `Name` variant in `block_py/mod.rs`, while `name_binding` turns only some `CoreBlockPyExpr::Name(...)` loads into `LoadGlobal`, `LoadCell`, or other storage-aware operations in `passes/name_binding.rs`.
  - The desired end state is for expression-level name loads to always be represented as an explicit operation, e.g. `LoadName`, with later passes refining that operation into `LoadCell`, `LoadGlobal`, or another storage-resolved form. That removes the split between “some loads are a variant, some loads are an operation.”
  - A safe implementation order is:
    - Add a `LoadName<N>` operation node alongside the other operation structs in `block_py/operation.rs`, with helper name text chosen explicitly and with name-mapping support like `LoadCell`.
    - Teach the pretty-printer, semantics helpers, and any direct operation pattern matches about `LoadName`, but initially keep the existing `Name` variants alive.
    - Add constructor helpers on the `CoreCallLikeExpr` / expr types so lowering code can emit `LoadName` directly instead of `Expr::Name`, and convert the obvious load-construction sites first.
    - Update `NameBindingMapper::map_expr` so the load-resolution logic matches `OperationDetail::LoadName(...)` instead of `CoreBlockPyExpr::Name(...)`, rewriting it into `LoadCell`, `LoadGlobal`, or a resolved local load form as appropriate.
    - Decide separately what to do with store/delete targets. `BlockPyAssign.target` and `BlockPyDelete.target` are name-typed today and should likely stay that way; this change is about load expressions, not statement targets.
    - After name-binding and the BlockPy-native semantics helpers no longer depend on `Expr::Name`, remove the `Name` variants from the `*Expr` enums and simplify `MapExpr` / `TryMapExpr` implementations so they only distinguish literals, calls, and operations.
  - The main things to audit before deleting the variant are:
    - `ImplicitNoneExpr` and other places that currently synthesize sentinel names like `__dp_NONE` as `Expr::Name`.
    - `block_py::semantics` helper methods that currently treat `Self::Name(...)` as the generic “loaded name” case.
    - any remaining tests or debug renderers that assume loads show up as the enum variant rather than an operation detail.

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
- Move refcount management out of `soac-eval` and into a new explicit pass in `rewrite_module`.
  - Planning note:
    - The current JIT path in `soac-eval` still owns a large amount of `incref` / `decref` insertion and runtime helper wiring (`dp_jit_incref`, `dp_jit_decref`), which makes ownership of reference semantics backend-local instead of pipeline-visible.
    - The desired end state is for refcount ownership to become an explicit lowered-module pass in `rewrite_module`, so later backends consume already-refcount-annotated IR instead of each backend re-deriving those rules.
    - A good first pass is to identify the minimal IR annotation or explicit stmt/term forms needed for retain/release edges, then move the current JIT-only reference-management decisions behind one driver-visible transform boundary.
- Merge `ast_to_ast::semantic` and `block_py::semantics` and `ast_symbol_analysis`, `dataflow`, and `callable_semantic.rs`
  - Planning note:
    - The current semantic facts are split across AST-side and BlockPy-side modules even though both are trying to model the same binding/storage/capture concepts at different points in the pipeline.
    - The desired end state is to have one semantic ownership point, with a clear boundary for what is still AST-shaped versus what has already been lowered to BlockPy, instead of duplicating concepts and helper logic across two modules.
    - A good first pass is to inventory which semantic data types and queries are duplicated or nearly duplicated, then decide whether the merged module should live at the AST boundary, the BlockPy boundary, or as a shared layer consumed by both.
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

- Merge simplify into the BlockPy pass and run it bottom-up so it is one-shot.
  - Planning note:
    - `blockpy_expr_simplify` is currently a separate pass boundary after semantic BlockPy construction, even though conceptually it is just finishing the lowering of expressions into core BlockPy form.
    - The likely simplification is to fold that work into the BlockPy lowering pass itself and run expression reduction bottom-up, so expressions only cross one lowering seam instead of first building semantic BlockPy exprs and then revisiting them.
    - A good first pass is to list which invariants `blockpy_expr_simplify` currently enforces for later passes, then check whether those can be guaranteed directly during semantic BlockPy construction without losing the current clear boundary for invalid leaked expr shapes.
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

## Directly build Operations in simplify_expr

- Planning note:
  - Today `impl From<Expr> for CoreBlockPyExprWithAwaitAndYield` in `passes/blockpy_expr_simplify/mod.rs` is mixed. `Add` already builds a `BinOpKind::Add` operation directly, but `Attribute`, `Subscript`, `UnaryOp`, most `BinOp`, and simple `Compare` still synthesize helper-call syntax like `__dp_getattr(...)` or `__dp_lt(...)` and then immediately reparse that string name through `lower_core_call_expr_with_meta` and `operation_by_name_and_args`.
  - The clean end state is for syntax-origin operator shapes to lower straight to `OperationDetail` values, while `operation_by_name_and_args` remains only for actual helper calls that enter the core boundary as calls, such as explicit `__dp_make_function(...)` or helper-shaped setup output from earlier passes.
  - A safe implementation order is:
    - Add direct constructor helpers in `blockpy_expr_simplify/mod.rs` for the operation families that still round-trip through helper strings, e.g. unary-op, binop, ternary-op, getattr, getitem, and simple compare helpers that take explicit kinds plus metadata.
    - Rewrite the `Expr::Attribute`, `Expr::Subscript`, `Expr::UnaryOp`, non-`Add` `Expr::BinOp`, and single-op `Expr::Compare` arms in `impl From<Expr>` to call those constructors directly instead of `py_expr!`, `make_binop`, or `make_unaryop`.
    - Keep evaluation order identical by continuing to lower child expressions in source order before constructing the `Operation`; for `In` and `NotIn`, preserve the existing operand reversal used by `Contains`, and keep `NotIn` as `UnaryOpKind::Not` wrapped around `BinOpKind::Contains`.
    - After those syntax-origin arms are direct, shrink `operation_by_name_and_args` to the remaining helper-call-origin cases, and consider renaming it to make that narrower responsibility obvious.
    - Leave tuple/list/set/dict/slice helper-call lowering alone in the first pass, since those are still intentionally represented as named calls today and are not part of the operation family being cleaned up here.
  - Verification focus:
    - Extend the existing `blockpy_expr_simplify` tests to cover direct lowering for the moved families and keep the current operation-kind assertions.
    - Re-run `just test-all` and specifically watch `core_eval_order`, `blockpy_expr_simplify`, and BB-string normalization tests for any behavioral drift from changed lowering order or metadata stamping.

## Expose per-module constants to codegen

- Planning note:
  - There are two different constant/data channels that codegen needs, and they should stop sharing one access story:
    - Python runtime constants: real `PyObject` values such as strings, bytes, large ints, kw-name tuples, and any other per-module objects that generated code wants to load or call against.
    - Internal lowering data: Rust-only metadata such as `BlockPyModule<CodegenBlockPyPass>`, function tables, storage layouts, block plans, and any future compile-time descriptors.
  - Today those are split inconsistently:
    - Python constants are mostly re-materialized ad hoc in JIT codegen, for example `emit_owned_string_constant` and the bytes/int/float literal paths in `soac-eval/src/jit/intrinsics.rs` and `soac-eval/src/jit/mod.rs`.
    - Internal data is stored per module in `_soac_ext` module state for the root `BlockPyModule`, but function-level lookup still escapes through the global `BB_FUNCTION_REGISTRY` in `soac-eval/src/jit/planning.rs`, keyed by `(module_name, function_id)`.
  - The clean split is:
    - `CompileSession` owns all internal lowering data and typed ids.
    - `_soac_ext` module state owns the realized Python constant pool for that module, plus a typed handle back to the owning `CompileSession` / module id.
    - Codegen consumes one explicit module-codegen context that can answer both “give me Python constant #k” and “give me internal function/module metadata for id X” without stringly global lookup.
  - A safe implementation order is:
    - Extend `CompileSession` from the current copyable id wrapper into an owning `Arc`-backed session object with typed registries such as `ModuleId`, `FunctionId`, and later `PythonConstId`.
    - Move `BlockPyModule` / function-table ownership into that session. `create_module` should lower once, register the lowered module in the session, and store only `(session handle, module id, module/package names)` in `_soac_ext` module state.
    - Replace `register_clif_module_plans` / `lookup_blockpy_function(module_name, function_id)` with session-scoped lookup APIs, so `make_bb_function`, `make_bb_generator`, and `exec_module` stop resolving through module-name strings and the global `BB_FUNCTION_REGISTRY`.
    - Introduce an explicit Python constant pool in the codegen IR boundary:
      - lower Python-constant-bearing sites to typed constant references instead of re-decoding from raw bytes or rebuilding names inline;
      - start with string-ish cases that are already obvious, e.g. attribute names, global-name loads, bytes/string literals, and decode-literal helpers;
      - handle larger or composite constants later, e.g. big ints, kw-name tuples, and any tuple/dict literal fragments that should become pooled objects.
    - Realize that Python constant pool once per module in `_soac_ext` module state, not in the Rust-only `CompileSession`. That keeps Python references attached to the module object that owns their lifetime.
    - Because module state would then hold `PyObject` references, reintroduce real `m_traverse` / `m_clear` handling in `soac-eval/src/module_type.rs` so GC can see and clear those Python constant references safely.
    - Add a small runtime accessor layer for JIT codegen, for example “load python const by id from module state / owner”, instead of emitting bespoke bytes-to-string decode logic at every site.
  - Design constraints to preserve:
    - Keep evaluation order unchanged. Pooling a constant is only a representation change; it must not move side effects or accidentally share objects whose identity is supposed to be fresh.
    - Keep pure Rust internal metadata separate from `PyObject` ownership. `CompileSession` should not become a hidden Python-GC root unless that is explicitly intended.
    - Avoid using `module.__dict__` as the internal-data store. Module dict values are Python-visible runtime semantics; the internal registries should stay in typed session/module-state storage.
  - Good first slice:
    - session-own the lowered module/function lookup path first, replacing the global registry;
    - then pool/load attribute-name and global-name strings through module state;
    - only after that decide which remaining literal categories should stay as inline materialization fast paths versus moving into the module Python constant pool.

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
- Should we linearize in the BlockPy pass so the whole block structure is uniform?
- Clean up the conversions and related glue in `block_py/mod.rs`.
- Compute `ClosureLayout` in `name_binding`, and keep all closure data semantic before that.
- Add a pass for specific storage decisions, closure slot offsets, and stack offsets.
- Use Ruff for scope analysis and see if it can be computed once and preserved through transform layers.
