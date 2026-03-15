# BlockPy Migration Plan

## Goal

Introduce a new transform-internal IR between Ruff AST and `BbModule`:

1. `Ruff AST -> Ruff AST`
2. `Ruff AST -> BlockPy`
3. `BlockPy -> BbModule`
4. `BbModule -> BbModule`

The intent is to keep semantic control-flow lowering in a Python-shaped IR, and leave true basic-block formation to a later step.

## Design constraints

- `BlockPy` stays close to Python structure.
- `BlockPy` explicitly represents labels and jumps.
- `BlockPy` allows implicit fallthrough between labeled blocks.
- `BlockPy` does **not** require every block to be a linear sequence ending in an explicit terminator.
- `BbModule` remains the codegen-facing IR where blocks are linear and end in an explicit terminator.
- `await -> yield from` remains a pass, not a vertical subsystem.
- `br_table` should eventually become a first-class primitive alongside `jump`, but that is deferred until the end of the migration.

## Proposed BlockPy shape

This is the current target shape for the first version:

- `BlockPyModule`
  - `prelude: Vec<BlockPyStmt>`
  - `functions`
  - `module_init`
- `BlockPyFunction`
- `BlockPyBlock { label: BlockPyLabel, body: Vec<BlockPyStmt> }`
- `BlockPyStmt`
  - `Pass`
  - `Assign`
  - `Expr`
  - `Delete`
  - `FunctionDef(ruff::StmtFunctionDef)` as an initial passthrough shape
  - `If { test, body, orelse }`
  - `For(ruff::StmtFor)` as an initial passthrough shape
  - `Jump(BlockPyLabel)`
  - `Return(expr?)`
  - `Raise { exc }`
  - `Try { body, handlers, orelse, finalbody }`

Notes:
- `If` remains statement-shaped in `BlockPy`.
- `BlockPyIf.body` / `orelse` own nested `BlockPyBlock` regions, so nested control-flow lowering can introduce labels and `Jump` uniformly.
- `Jump` is the only explicit branch primitive initially.
- To get the first `Ruff AST -> BlockPy` pass wired quickly, `BlockPyStmt` may temporarily carry a small, explicit list of Ruff stmt nodes directly. These are intended to be progressively narrowed away rather than kept permanently.
- `br_table` is intentionally deferred. When added later, it should be introduced as another primitive at the `BlockPy` level rather than being baked into earlier steps.
- Labels in `BlockPy` should use a dedicated wrapper type instead of bare `String`, so label identity stays typed and can carry metadata later without another broad churn.
- Exceptions in `BlockPy` should stay structured and Python-like. The `Try` node is intentionally higher level than the current `BbModule`/`TryJump` representation; later phases can flatten it into label-level control flow and exception-edge bookkeeping.
- `BlockPyFunction` should stay semantic and minimal:
  - keep `bind_name`, `qualname`, `binding_target`, `kind`, and Ruff `Parameters`
  - do not carry codegen-facing fields like `display_name`, `entry_label`, `param_specs`, or generator closure metadata
- `BlockPyAssign` / `BlockPyDelete` should use single-name targets only; earlier Ruff-to-Ruff passes should continue lowering complex assignment/delete shapes before `BlockPy`

## Non-goals for the first phase

- No change to runtime behavior.
- No change to `BbModule` semantics.
- No broad refactor of the generator/coroutine lowering yet.
- No attempt to make `BlockPy` public or executable.

## Migration steps

### Step 1: Introduce `BlockPy` scaffolding

Status: completed

Add a transform-internal `BlockPy` module with the initial IR type definitions only.

Scope:
- Add `dp-transform/src/basic_block/block_py.rs`
- Export it from `dp-transform/src/basic_block/mod.rs`
- Define the initial `BlockPy*` structs/enums
- Do not wire any pipeline code to use it yet

Why first:
- It establishes the new phase boundary without risking behavior changes.
- It makes later refactors concrete instead of speculative.

Questions to revisit:
- Should `BlockPy` carry Ruff `Expr` directly in v1 everywhere, or do `Assign` / `Delete` need narrower target/value wrappers earlier than the rest?

### Step 2: Extract the current private lowering output into `BlockPy`

Status: in progress

Sub-step 2a: completed

- Added a validation-only `Ruff AST -> BlockPy` conversion path.
- Wired it into the transform driver before `Ruff AST -> BbModule`.
- The result is still discarded for now; the goal is to verify that the current post-simplification AST can be expressed as `BlockPy`.
- `BlockPyStmt` currently allows a small explicit passthrough set of Ruff stmt nodes (for example `FunctionDef`, `ClassDef`, `TypeAlias`, `While`, `For`, `With`, `Import`, `ImportFrom`, `Break`, `Continue`) so this validation pass can land before those forms are narrowed further.
- `BlockPyModule` now also carries a top-level `prelude`, so transformed helper imports and other executable module-level setup can be represented without prematurely forcing everything into `_dp_module_init`.

Sub-step 2b: completed

- The validation-only `Ruff AST -> BlockPy` path now succeeds across the current transformed test suite.
- `BlockPyModule.prelude` is the compatibility bucket for executable top-level setup during this phase.
- Remaining post-transform Ruff stmt shapes that still reach `BlockPy` are accepted explicitly as passthrough `BlockPyStmt` variants rather than being rejected.
- No runtime or `BbModule` behavior changes were made in this step.

Change the current internal output of `ast_to_bb` from private `Block` / `Terminator`-style structs into `BlockPyFunction` / `BlockPyBlock`.

Scope:
- Move private lowering data structures out of `ast_to_bb/mod.rs`
- Translate terminator-bearing blocks into `BlockPy` blocks with explicit control-flow statements in the body
- Preserve all existing behavior

Why:
- This makes the current internal phase explicit.
- It is the compatibility bridge before `blockpy_to_bb` exists as a real sibling module.

Questions to revisit:
- Whether `Yield` should remain an internal lowering-only statement in this transitional step, or be lowered away before entering `BlockPy`
- Which currently passthrough Ruff stmt nodes should be narrowed first, and whether that narrowing belongs in earlier Ruff-to-Ruff passes or directly in `Ruff AST -> BlockPy`
- Whether `BlockPyModule.prelude` should eventually collapse into an explicit `_dp_module_init`-style function form before `BlockPy -> BbModule`, or remain a true top-level sequence until later

### Step 3: Add `blockpy_to_bb`

Status: in progress

Sub-step 3a: completed

- Added a new sibling module: `basic_block/blockpy_to_bb/`
- Moved the final private-CFG-to-`BbBlock` shaping out of `ast_to_bb/mod.rs` into that module.
- This is currently a compatibility bridge: it still consumes the existing internal private CFG (`Block`, `Terminator`) rather than true `BlockPy`.
- No semantic or runtime behavior changed; the goal of this sub-step is just to isolate the late `* -> BbModule` shaping boundary.

Sub-step 3b: completed

- `blockpy_to_bb` now consumes a compatibility `BlockPyBlock` form instead of lowering straight from the private CFG.
- The current `ast_to_bb` path first converts the private `Block` / `Terminator` output into `BlockPyBlock`, then lowers that into `BbBlock`.
- This is still a transitional bridge: the source of truth is the existing private CFG, and `BlockPy` is only a compatibility target at this stage.
- No semantic or runtime behavior changed; the value of this step is that the `BlockPy -> BbModule` boundary is now exercised in the live path.

Sub-step 3c: completed

- `LoweredFunction` now carries `Vec<BlockPyBlock>` instead of `Vec<BbBlock>`.
- Closure-backed generator factory helpers now emit `BlockPyBlock` directly.
- The final `BlockPyBlock -> BbBlock` shaping happens only in `blockpy_to_bb`.
- The compatibility shim from the older private CFG still exists for the rest of the function body lowering, so `ast_to_bb` is now mixed-mode rather than fully converted.
- The regression cluster from this step came from missing block-param metadata on closure-backed factory blocks; that is fixed by carrying explicit `block_params` / `exception_edges` alongside `LoweredFunction.blocks`.

Sub-step 3d: completed

- The remaining old `Block` / `Terminator` compatibility IR now lives in a dedicated `ast_to_bb/private_cfg.rs` module instead of being defined inline in `ast_to_bb/mod.rs`.
- This is still a transitional representation, but it is now a named compatibility layer rather than hidden inside the main lowering module.
- No semantic or runtime behavior changed in this step; it is a preparatory cleanup to make the next direct-`BlockPy` conversions easier to reason about.

Sub-step 3e: completed

- The simplest `lower_stmt_sequence()` terminal producers now construct BlockPy-style terminal statements through the compatibility helper instead of directly instantiating `Terminator`.
- This now covers:
  - plain `return`
  - plain `raise`
  - `break`
  - `continue`
  - the final fallthrough jump
  - `while` test / linear-entry blocks
  - `for` setup / loop-check / assign / async-fetch continuation blocks
  - the top-level `try` entry node in `lower_stmt_sequence()`
- The old private CFG still exists, but the producer side is now shrinking from the statement-lowering path rather than only being wrapped later.

Sub-step 3f: completed

- The simple generator dispatch/support blocks in `try_lower_function()` now also use BlockPy-style terminal construction through the compatibility helper.
- This now covers the done/invalid/uncaught support blocks and the send/throw dispatch tables and precheck blocks.
- Generator lowering is still mixed-mode overall, but the dispatch spine no longer needs to instantiate raw `Terminator` values directly.

Sub-step 3g: completed

- The non-`Yield` support blocks around `yield` / `yield from` in `lower_stmt_sequence()` now also construct BlockPy-style terminal statements through the compatibility helper.
- This now covers:
  - the resume-exception dispatch blocks around bare `yield`
  - the assignment/return continuation and dispatch blocks around `yield from`
  - the resume-return dispatch blocks around `return yield`
- The actual `Yield` terminator remains on the compatibility IR for now, which keeps this slice low-risk while continuing to shrink direct `Terminator` construction.

Sub-step 3h: completed

- Added a transitional `BlockPyStmt::LegacyYield` compatibility variant so the remaining raw `Terminator::Yield` producer sites could also move through `compat_block_from_blockpy()`.
- `ast_to_bb/mod.rs` no longer directly instantiates raw `Block { .., terminator: .. }` values; all producer sites now construct compatibility blocks through BlockPy terminal statements.
- The old `private_cfg` representation still exists, but it is now only the compatibility storage layer behind `compat_block_from_blockpy()` rather than something the lowering code constructs directly.

Sub-step 3i: completed

- Producer-side lowering signatures in `ast_to_bb/mod.rs` now use `Vec<BlockPyBlock>` directly:
  - `lower_stmt_sequence(...)`
  - `lower_yield_from_direct(...)`
  - the initial `try_lower_function(...)` block construction path
- `compat_block_from_blockpy(...)` now constructs `BlockPyBlock`, not the old private CFG block.
- The old `private_cfg::Block` form is now reintroduced only at the late compatibility boundary via `compat_lower_blockpy_blocks_to_private_cfg(...)`, immediately before the still-legacy helper/dataflow passes.
- One local compatibility bridge remains inside `lower_stmt_sequence(...)` for `rewrite_region_returns_to_finally(...)`, which still mutates the old `Block` representation. That helper is now the main remaining reason `private_cfg` still appears in producer-side code at all.

Sub-step 3j: completed

- The simple CFG-shaping helpers now operate on `BlockPyBlock` in the live path:
  - `fold_jumps_to_trivial_none_return_blockpy(...)`
  - `fold_constant_brif_blockpy(...)`
  - `prune_unreachable_blockpy_blocks(...)`
  - `relabel_blockpy_blocks(...)`
- The older private-CFG copies of those simple helpers have been removed.
- One narrower private-CFG relabeling helper still remains:
  - `apply_label_rename(...)`
  - it is still used later in `try_lower_function(...)` after the temporary conversion back to `private_cfg`, specifically for the remaining legacy generator relabeling path.
- This leaves the compatibility boundary clearer:
  - simple fold/prune/relabel work now happens on `BlockPy`
  - only the later generator/exception helper path still depends on `private_cfg`

Sub-step 3k: completed

- `try_lower_function(...)` now keeps `BlockPy` through the late generator relabel/dispatch build as well.
- The temporary conversion back to `private_cfg` is delayed until after:
  - resume-label collection
  - resume/internal relabeling
  - resume-order computation
  - generator dispatch/support block insertion
- As a result, the older private-CFG relabel helper `apply_label_rename(...)` is no longer needed and has been removed.
- The dead private helper `compat_private_block_from_blockpy(...)` is also removed.
- The remaining `private_cfg` dependency is now later still:
  - deleted-name / unbound-local rewrite
  - exception-edge computation
  - block-param computation
  - closure-cell rewrite
  - other legacy helper/dataflow passes after the final temporary conversion

Sub-step 3l: completed

- `rewrite_region_returns_to_finally(...)` now has a native `BlockPy` version used directly from `lower_stmt_sequence()`.
- The local producer-side compatibility bridge (`with_private_cfg_blocks(...)`) is removed.
- The old private-CFG version of `rewrite_region_returns_to_finally(...)` is removed as dead code.
- This means the remaining `private_cfg` dependency is no longer in `lower_stmt_sequence()` itself; it is concentrated later in `try_lower_function()` after the final temporary conversion for legacy dataflow/exception helpers.

Sub-step 3m: completed

- `compute_exception_edge_by_label(...)` now has a native `BlockPy` version used from `try_lower_function()` before the final temporary conversion back to `private_cfg`.
- The old private-CFG version of `compute_exception_edge_by_label(...)` is removed as dead code.
- This moves another label/region analysis step onto `BlockPy` and further narrows the remaining late `private_cfg` dependency.
- The remaining `private_cfg` dependency is now later still:
  - deleted-name / unbound-local rewrite
  - block-param computation
  - closure-cell rewrite
  - other legacy helper/dataflow passes after the final temporary conversion

Sub-step 3n: completed

- `build_extra_successors(...)` now has a native `BlockPy` version used from `try_lower_function()` before the final temporary conversion back to `private_cfg`.
- The old private-CFG version of `build_extra_successors(...)` is removed as dead code.
- This moves another CFG-analysis step onto `BlockPy` and further narrows the remaining late `private_cfg` dependency.
- The remaining `private_cfg` dependency is now later still:
  - deleted-name / unbound-local rewrite
  - block-param computation
  - closure-cell rewrite

Sub-step 3u: completed

- `blockpy_to_bb` now lowers terminal control flow directly from `BlockPyStmt` to `BbTerm` instead of reconstructing the legacy `Terminator` shape first.
- Terminal-expression simplification for:
  - `BranchIf`
  - `BranchTable`
  - `Raise`
  - `LegacyYield`
  - `Return`
  now runs directly on the terminal `BlockPyStmt` before `BbTerm` creation.
- The remaining `Terminator` references in `blockpy_to_bb` are now limited to the old `Block -> BlockPy` compatibility conversion, not the live `BlockPy -> BbModule` lowering path.

Sub-step 3v: completed

- The dead `private_cfg` compatibility/storage layer is removed entirely.
- The unused old `Block` / `Terminator` conversion helpers are deleted.
- The stale `Terminator`-only simplification/lowering helpers in `terminator_lowering.rs` are deleted.
- `ast_to_bb` no longer re-exports `Block` / `Terminator`.
- At this point, the live path is fully:
  - `Ruff AST -> BlockPy`
  - `BlockPy -> BbModule`
  - `BbModule -> BbModule`

Sub-step 3o: completed

- `compute_block_params(...)` and `ensure_try_exception_params(...)` now have native `BlockPy` versions used from `try_lower_function()`.
- To preserve behavior, deleted-name rewriting still runs first on the compatibility CFG; the rewritten blocks are then converted back to `BlockPy` for liveness/block-param analysis.
- This moves the main remaining late dataflow computation onto `BlockPy` without changing ordering-sensitive semantics.
- The remaining `private_cfg` dependency is now later still:
  - deleted-name / unbound-local rewrite
  - closure-cell rewrite
  - other legacy helper/dataflow passes after the final temporary conversion

Sub-step 3p: completed

- `collect_state_vars(...)` now consumes `BlockPy` blocks instead of `private_cfg` blocks.
- Added `collect_injected_exception_names_blockpy(...)` so state-var analysis no longer depends on `LegacyTryJump` through the old CFG form.
- To preserve behavior, deleted-name rewriting still runs first on the compatibility CFG; the rewritten blocks are then converted back to `BlockPy` for state-var and block-param analysis.
- The remaining `private_cfg` dependency is now later still:
  - deleted-name / unbound-local rewrite
  - closure-cell rewrite
  - other legacy helper/dataflow passes after the final temporary conversion

Sub-step 3q: completed

- `rewrite_sync_generator_blocks_to_closure_cells(...)` now has a BlockPy-native form used from `try_lower_function()`.
- To preserve behavior, deleted-name rewriting still runs first on the compatibility CFG; the rewritten blocks are then converted back to `BlockPy` for state-var collection, block-param analysis, and the sync-generator closure-cell rewrite.
- The rewritten BlockPy blocks are only converted back to `private_cfg` after that point for the remaining legacy post-rewrite helpers.
- The remaining `private_cfg` dependency is now later still:
  - deleted-name / unbound-local rewrite
  - other legacy helper/dataflow passes after the final temporary conversion

Sub-step 3r: completed

- The remaining post-rewrite generator helpers now run directly on `BlockPy`:
  - yield/return completion rewriting
  - uncaught cleanup-cell mutation
- `try_lower_function()` no longer converts the rewritten `BlockPy` graph back to `private_cfg` for those late generator steps.
- The second late `private_cfg -> BlockPy -> private_cfg` round-trip is gone.
- The remaining `private_cfg` dependency is now exactly the early deleted-name / unbound-local rewrite:
  - convert to `private_cfg`
  - run deleted-name rewriting
  - convert back to `BlockPy`

Sub-step 3s: completed

- `rewrite_deleted_name_loads(...)` now runs directly on `BlockPy` blocks.
- The last live `private_cfg` round-trip inside `try_lower_function()` is gone.
- `try_lower_function()` now stays on `BlockPy` from semantic lowering through:
  - deleted-name / unbound-local rewriting
  - state-var collection
  - block-param computation
  - sync-generator closure-cell rewriting
  - generator yield/return completion rewriting
- `private_cfg` remains only as a transitional compatibility/storage module, not as a live phase in `try_lower_function()`.

Sub-step 3t: completed

- The producer-side `compat_block_from_blockpy(...)` helper now lives directly in `ast_to_bb/mod.rs`, so producer code no longer depends on `private_cfg` for block construction.
- Dead private-CFG-only analysis helpers have been removed:
  - old state-var/closure rewrite helpers
  - old deleted-name-dependent use/def helpers
  - old terminator load-name helpers
- The remaining `private_cfg` surface is now strictly compatibility/storage:
  - compatibility conversions between `BlockPy` and the old `Block` / `Terminator` types
  - no live lowering or analysis step depends on it anymore

Create a new sibling module that performs the real basic-block shaping:

- explicit fallthrough resolution
- block splitting
- terminator formation
- live-set / block-param analysis entrypoint

Scope:
- New module under `dp-transform/src/basic_block/`
- `BlockPy -> BbModule`
- No semantic lowering in this phase

Why:
- This isolates the true “basic block” work in one place.

Questions to revisit:
- Whether `blockpy_to_bb` should own all block-param computation, or whether some helper analysis should stay shared

### Step 4: Move generator-family semantic lowering to `Ruff AST -> BlockPy`

Status: pending

Move generator/coroutine/async-generator lowering to emit BlockPy-level control flow instead of directly shaping `BbModule`.

Scope:
- `yield`
- `yield from`
- resume dispatch
- closure-backed generator factory lowering

Why:
- This is the largest current vertical in `ast_to_bb`.
- It is the clearest win from introducing `BlockPy`.

Questions to revisit:
- Whether hidden resume dispatch should use only `Jump` + `If` initially or whether adding `br_table` earlier would materially simplify generator lowering

### Step 5: Move exception / with / finally semantic lowering to `Ruff AST -> BlockPy`

Status: pending

Keep `Try` structured in BlockPy, then let `blockpy_to_bb` turn it into explicit BB control flow where needed.

Why:
- Exceptions are a semantic lowering problem first, and a basic-block problem second.

Questions to revisit:
- How much explicit label ownership nested `Try` regions should have versus using a more statement-shaped lowering before the final `BlockPy -> BbModule` split.

### Step 6: Make the pipeline explicit at `basic_block` module scope

Status: pending

The target top-level shape is:

- `rewrite_ruff_to_blockpy(...)`
- `lower_blockpy_to_bb(...)`
- `prepare_bb_module_for_jit(...)`
- `prepare_bb_module_for_codegen(...)`

Why:
- Keeps phase order visible in one place.
- Makes future feature work easier to place in the correct phase.

### Step 7: Add `br_table` to `BlockPy`

Status: pending

This is intentionally deferred until the rest of the migration is stable.

Scope:
- Add `br_table` as a first-class BlockPy primitive alongside `jump`
- Use it where it materially simplifies generator resume dispatch and similar indexed dispatch shapes

Why deferred:
- It is not required to establish the phase split.
- Adding it too early would create more moving pieces while the new IR boundary is still being established.

## Completed

- Split `BbModule -> BbModule` passes into `basic_block/bb_passes/`
- Lifted `basic_block` pipeline entrypoints:
  - `prepare_bb_module_for_jit(...)`
  - `prepare_bb_module_for_codegen(...)`
- Step 1 completed:
  - added transform-internal `BlockPy` IR scaffolding in `dp-transform/src/basic_block/block_py.rs`
  - exported `block_py` from `dp-transform/src/basic_block/mod.rs`
  - no pipeline behavior changes yet
- Step 2a completed:
  - added `basic_block/ruff_to_blockpy/`
  - wired a validation-only `rewrite_ast_to_blockpy_module(...)` call into `transform::driver`
  - the `BlockPy` result is currently discarded after conversion succeeds
- Top-level function-body `while` is now lowered in `ruff_to_blockpy` into labeled `BlockPy` blocks with terminal `If + Jump` loop backedges and explicit after/else blocks.
- Raw `While` has been removed from the `BlockPy` stmt surface; if one reaches the stmt-list lowering boundary, that is treated as a bug.
- Raw `Break` / `Continue` have been removed from the `BlockPy` stmt surface; at the Ruff AST -> BlockPy boundary they are rewritten directly to `Jump` when loop context is known, and otherwise are treated as a bug.
- Dead high-level stmt surface that should already be gone before `BlockPy` has been removed and now fails fast if encountered:
  - `Assert`
  - `Match`
  - `AnnAssign`
  - `AugAssign`
  - `TypeAlias`
  - `Import`
  - `ImportFrom`
  - `ClassDef`

## Open questions to revisit

1. Should `BlockPy` use Ruff `Expr` directly everywhere in v1, or should some lowered control-flow expressions get their own narrower representation early?
2. Do we want `Coroutine` as a separate `BlockPyFunctionKind`, or should coroutine-via-generator remain represented through generator-style lowering plus metadata?
3. Should `BlockPyTry.body` / `orelse` / `finalbody` own nested `BlockPyBlock` lists directly, or should there be an intermediate structured statement form before final block ownership is assigned?
4. When `br_table` is added, should it be introduced directly into `BlockPy`, or first as a transform-local helper lowered immediately to `If`/`Jump`?
