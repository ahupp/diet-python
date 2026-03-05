# REDESIGN v2

## Goals

Order of priority:

1. Make the codebase easier to understand.
2. Make it easier to add new features safely.
3. Preserve or improve performance as a consequence of cleaner structure.

Constraints:

- Preserve Python semantics and evaluation order.
- Keep AST traversal via `Transformer`-style visitors.
- Do not add new minimal-AST variants unless explicitly requested.
- Prefer transform-time structure over runtime patching where practical.

## Executive Summary

The repository has converged on a basic-block-centric architecture, but it still carries a second execution architecture (`min_ast` + tree-walk evaluator) and a large amount of bridge code between:

- Ruff AST rewriting
- BB IR generation
- BB normalization for codegen
- CLIF planning/codegen
- Python runtime helper construction in `__dp__.py`
- eval-frame/code-extra plumbing in `soac-eval`

The main simplification opportunity is to make the BB IR the single authoritative semantic boundary for execution, then make every other layer smaller and more explicit around that boundary.

The second major opportunity is to replace stringly-typed and late-normalized interfaces with typed IR, typed plan objects, and one-owner passes.

## Codebase Map

### Main crates and modules

- `dp-transform/`
  - Parses Python with Ruff.
  - Runs source-to-source lowering passes.
  - Builds BB IR.
  - Still also exposes `min_ast`.
  - Main orchestration: `dp-transform/src/lib.rs`, `dp-transform/src/transform/driver.rs`, `dp-transform/src/basic_block/ast_to_bb/mod.rs`, `dp-transform/src/basic_block/bb_ir.rs`.

- `soac-pyo3/`
  - Python extension entrypoint.
  - Exposes transform helpers to Python.
  - Registers BB/CLIF plans on transform.
  - Performs JIT preflight and some module execution orchestration.
  - Main files: `soac-pyo3/src/lib.rs`, `soac-pyo3/src/eval.rs`.

- `soac-eval/`
  - Python eval-frame hook integration.
  - Code-extra registration and lookup.
  - CLIF planning and JIT execution.
  - Still contains the tree-walk `min_ast` runtime.
  - Main files: `soac-eval/src/code_extra.rs`, `soac-eval/src/tree_walk/eval.rs`, `soac-eval/src/jit/mod.rs`, `soac-eval/src/jit/planning.rs`, `soac-eval/src/jit/exception_pass.rs`.

- `__dp__.py`
  - Runtime helper surface exposed to transformed Python.
  - Operator helpers, locals/globals/cell helpers, exception helpers, function/generator/coroutine constructors, import/class helpers, JIT wrapper glue.
  - This is both a semantic runtime layer and a compatibility/bootstrapping layer.

- `diet_import_hook/`
  - Import-time transformation loader.
  - Executes transformed source, then runs `_dp_module_init()`.

- `tests/`, `test_sets/`, `scripts/`
  - Integration/regression coverage.
  - CPython suite wrappers.
  - Current architecture is validated mostly through behavior tests rather than stage-specific invariants.

- `web/` and `dp-transform/src/web_inspector.rs`
  - Debug/inspection UI for transformed source, BB IR, and CLIF rendering.

- `soac-runtime/`
  - Standalone experimental crate, not part of the normal workspace execution path.

### Current large files / complexity hotspots

- `dp-transform/src/basic_block/ast_to_bb/mod.rs`
- `soac-eval/src/tree_walk/eval.rs`
- `soac-eval/src/jit/mod.rs`
- `__dp__.py`

These are the primary files to break up further.

## Current End-to-End Dataflow

### Import-hook / normal transformed execution path

1. `diet_import_hook.DietPythonLoader.source_to_code()` reads source and calls the pyo3 extension.
2. `soac-pyo3/src/lib.rs:transform_source_with_name()` calls `dp_transform::transform_str_to_ruff_with_options(...)`.
3. `dp-transform/src/transform/driver.rs:rewrite_module()` performs AST-level lowering and optionally emits `BbModule`.
4. `soac-pyo3` normalizes the BB module for codegen and registers CLIF plans with `soac_eval::jit::register_clif_module_plans(...)`.
5. The import hook compiles the transformed Python source and executes it.
6. The import hook runs `_dp_module_init()`.
7. Generated module-init code calls `__dp_def_fn`, `__dp_def_gen`, `__dp_def_async_gen`, etc., to create Python callables.
8. Those helpers in `__dp__.py` create placeholder Python functions/generator wrappers and register lazy CLIF wrapper metadata.
9. On first function execution, the eval-frame hook in `soac-eval/src/tree_walk/eval.rs` looks up code-extra metadata, compiles the CLIF wrapper if needed, and dispatches into the JIT path.
10. `soac-eval/src/jit/*` executes the CLIF-backed plan.

### Secondary / legacy execution path

There is still a parallel path through `min_ast` and the tree-walk interpreter:

1. `soac-pyo3/src/eval.rs:transform_to_min_ast(...)` lowers source to Ruff AST and then converts to `min_ast`.
2. `soac-eval/src/tree_walk/eval.rs` builds function data, code objects, scopes, and interprets `min_ast`.

Even when the CLIF path is primary, this second architecture still shapes the codebase heavily:

- `min_ast` remains public API inside the workspace.
- `soac-pyo3` still converts to `min_ast`.
- `soac-eval/src/tree_walk/eval.rs` still contains both wrapper-hook logic and a large interpreter/runtime.

### BB IR / planning path today

1. AST lowering emits `BbModule`.
2. `dp-transform/src/basic_block/codegen_normalize.rs` mutates BB IR again into a codegen-oriented shape.
3. `soac-eval/src/jit/planning.rs` lowers `BbModule` to `ClifPlan`.
4. `soac-eval/src/jit/exception_pass.rs` rewrites `TryJump` into explicit exception edges before planning.
5. `soac-eval/src/jit/mod.rs` turns `ClifPlan` into Cranelift IR and executable code.

This is directionally correct, but the plan/codegen boundary is still not the only execution boundary in the repository.

## Architectural Problems

## P1. Two execution architectures are still peers

The repository currently has:

- BB IR + CLIF path
- `min_ast` + tree-walk path

That makes control flow, callable creation, locals/closure handling, and annotation handling harder to reason about than they need to be. It also causes design work to get split between the two worlds.

## P2. BB IR is not yet the fully canonical backend-facing representation

`BbExpr` and related structures still accept shapes that are later normalized again:

- attribute/subscript/load forms become helper calls later
- string literals are normalized later
- `Await` and `Starred` still exist in BB expression space
- labels and exception metadata are still represented largely as strings

This creates representational drift between:

- `ast_to_bb`
- `codegen_normalize`
- `jit/planning`
- debug/web rendering

## P3. Exception lowering responsibility is split

Today:

- AST-to-BB lowering emits structured try control flow (`TryJump`)
- JIT-side `exception_pass` lowers it further
- runtime/JIT execution still has to care about exception-flow shape

This is the right direction, but the ownership boundary is not yet crisp enough. Exception structure should be lowered in one place with one clearly documented contract.

## P4. Callable materialization is spread across Python and Rust

Function creation today touches:

- transformed Python source
- `_dp_module_init`
- `__dp__.py` helper constructors
- code-extra registration
- `soac-pyo3`
- `soac-eval/tree_walk/eval.rs`

That makes seemingly small features like annotations, closure metadata, globals rebinding, and generator wrappers hard to place cleanly.

## P5. `__dp__.py` carries too many responsibilities

It currently mixes:

- semantic runtime helpers
- locals/globals/cell access
- function materialization
- generator/coroutine object model
- import-hook helper aliases
- JIT wrapper integration
- some legacy compatibility paths

This makes it hard to know which helpers are core semantics versus temporary bootstrapping.

## P6. Module boundaries do not reflect phase boundaries

Examples:

- `ast_to_bb/mod.rs` is simultaneously pre-lowering, CFG building, generator lowering, exception lowering, liveness, naming, metadata, and orchestration.
- `tree_walk/eval.rs` combines eval-frame interception, CLIF-wrapper setup, code-object handling, closure capture, scope runtime, and the legacy tree-walk interpreter.
- `jit/mod.rs` still mixes CLIF assembly, execution, caching, debug rendering, and specialized helper management.

## P7. Top-level repository structure still contains ambiguous or legacy surfaces

Examples:

- `soac-runtime/` is experimental and not part of the normal workspace path.
- `soac-codegen/` appears to be a leftover shell.
- web/debug rendering paths still need to consume the exact same canonical plan artifacts as the runtime, but that contract is not front-and-center.

## Target Architecture

The target architecture should be:

1. `dp-transform` lowers Python source into:
   - transformed/debuggable Python source
   - a canonical validated `BbModule`

2. `BbModule` is the single semantic execution boundary.

3. Any backend-specific preparation is explicit:
   - `BbModule -> PreparedBbModule` if needed
   - `PreparedBbModule -> ClifPlan`
   - `ClifPlan -> CLIF`
   - `ClifPlan -> compiled machine code`

4. Python callable materialization is driven by a small structured descriptor, not by source re-exec or broad helper pattern matching.

5. `soac-eval/tree_walk` is reduced to Python-interpreter integration glue:
   - eval-frame hook
   - code-extra
   - wrapper compilation/caching
   - possibly code-object construction
   - not a second execution engine

6. `__dp__.py` contains semantic helpers that transformed Python genuinely needs, plus a very small amount of bootstrap glue.

## Detailed Restructuring Plan

## Phase 1. Make the pipeline explicit in code

Add an explicit pipeline module in `dp-transform`, for example:

- `dp-transform/src/transform/pipeline.rs`

Responsibilities:

- define ordered pipeline stages
- document input/output invariants for each stage
- centralize stage sequencing
- make it obvious which stage is allowed to introduce or eliminate each construct

Concrete steps:

- Move the pass ordering currently embedded in `rewrite_module()` into a pipeline declaration.
- Give each stage a short contract comment.
- Add stage-specific validation hooks in debug/test builds.

Why first:

- This is the fastest readability win.
- It gives later refactors a stable structure.

Success condition:

- A new contributor can answer “where should feature X be lowered?” by reading one file.

## Phase 2. Make BB IR the only semantic execution boundary

This is the most important structural change.

Concrete steps:

- Stop treating `min_ast` as a peer execution IR.
- Reduce `soac-pyo3/src/eval.rs` so it no longer depends on `min_ast` for the main execution path.
- Move the remaining tree-walk interpreter code behind an explicit legacy/testing boundary, then delete it once parity is reached.
- Make transformed module execution depend on BB IR registration and wrapper metadata only.

Short-term shape:

- `LoweringResult` returns transformed Ruff AST string + canonical `BbModule`.
- `soac-pyo3` registers plans directly from `BbModule`.
- `tree_walk` keeps eval-frame/code-extra/wrapper plumbing only.

Why second:

- As long as `min_ast` remains a first-class runtime, every architectural decision has to be made twice.

Success condition:

- All normal transformed execution goes through `BbModule`.
- `min_ast` is either deleted or isolated as a legacy/internal-only compatibility layer.

## Phase 3. Narrow and validate the BB IR

The BB IR should be smaller, more typed, and closer to backend-ready.

Concrete steps:

- Add `basic_block/validate.rs`.
- Validate:
  - block IDs/edges
  - param arity
  - terminator ownership
  - exception-edge ownership
  - backend-ready expression shape
- Replace label-string-centric internals with stable IDs:
  - `FunctionId`
  - `BlockId`
  - keep debug labels as metadata only
- Reduce `BbExpr` to the smallest practical set:
  - names
  - numeric literals
  - bytes literals
  - calls
  - only keep starred/await if they truly must survive to BB, otherwise eliminate earlier
- Remove `to_expr()/from_expr()` round-tripping from normal backend paths where possible.

Important design rule:

- If a shape is “supposed to be lowered earlier”, it should not remain representable in backend-ready BB IR.

Success condition:

- Planning/codegen do not need to defensively rediscover whether the BB IR is in a usable shape.

## Phase 4. Move all backend normalization into explicit BB passes

`codegen_normalize.rs` is conceptually a pass, not a post-hoc convenience layer.

Concrete steps:

- Turn backend normalization into named BB passes under `dp-transform/src/basic_block/passes/`.
- Recommended passes:
  - `literal_normalize.rs`
  - `builtin_helper_lower.rs`
  - `call_shape_normalize.rs`
  - `exception_prepare.rs`
- Run those passes before plan registration, so every consumer sees the same canonical BB.

Why:

- Today the same BB can mean different things before and after normalization.
- Debug rendering, planning, and execution should all consume the same normalized representation.

Success condition:

- No consumer needs a private “one more normalize” step to understand the IR.

## Phase 5. Give exceptions one owner and one contract

Exception handling should be represented in two explicit forms only:

1. structured exception regions in early BB
2. explicit exception edges/handlers in backend-ready BB

Concrete steps:

- Keep early AST-to-BB lowering focused on semantic control-flow structure.
- Move the full “structured exception -> explicit exception edges” rewrite into one pass.
- Decide whether that pass belongs in:
  - `dp-transform/basic_block/passes/exception_lower.rs`, or
  - `soac-eval/jit/exception_pass.rs`

Recommended answer:

- Move it into `dp-transform/basic_block/passes/exception_lower.rs`.
- The JIT should consume backend-ready exception edges, not own semantic exception lowering.

Specific representation improvement:

- Represent exception edges as typed metadata on blocks/terminators rather than stringly `exc_target_label` / `exc_name` pairs.

Success condition:

- The JIT codegen layer never has to interpret structured try/finally semantics directly.

## Phase 6. Split `ast_to_bb` by phase, not syntax category alone

`dp-transform/src/basic_block/ast_to_bb/mod.rs` should become orchestration only.

Target submodules:

- `ast_to_bb/pre_lower.rs`
- `ast_to_bb/control_flow.rs`
- `ast_to_bb/functions.rs`
- `ast_to_bb/generators.rs`
- `ast_to_bb/async_lower.rs`
- `ast_to_bb/exceptions.rs`
- `ast_to_bb/locals.rs`
- `ast_to_bb/analysis_names.rs`
- `ast_to_bb/analysis_liveness.rs`
- `ast_to_bb/naming.rs`
- `ast_to_bb/metadata.rs`
- `ast_to_bb/validate.rs`

Rule:

- each file should own one idea, not one “misc bucket”.

Success condition:

- `ast_to_bb/mod.rs` reads like a driver, not like the implementation itself.

## Phase 7. Split `soac-eval/tree_walk` into “Python bridge” and “legacy evaluator”

The current name `tree_walk` is misleading because a large part of it is now Python runtime integration rather than tree walking.

Target structure:

- `soac-eval/src/py_bridge/code_extra.rs`
- `soac-eval/src/py_bridge/eval_frame.rs`
- `soac-eval/src/py_bridge/function_data.rs`
- `soac-eval/src/py_bridge/code_object.rs`
- `soac-eval/src/py_bridge/wrapper_compile.rs`
- `soac-eval/src/py_bridge/scope_runtime.rs`
- `soac-eval/src/legacy_eval/` only if the legacy interpreter still exists temporarily

Concrete steps:

- Move code-extra and eval-frame interception concerns under a “Python bridge” name.
- Move any remaining `min_ast` interpreter logic into an explicit legacy module.
- Delete the legacy module once Phase 2 is complete.

Success condition:

- `tree_walk/eval.rs` no longer exists as a 4000-line mixed-responsibility file.

## Phase 8. Continue splitting `soac-eval/jit` around durable boundaries

`planning.rs` and `specialized_helpers.rs` were the right first step. Continue until `jit/mod.rs` is orchestration only.

Target modules:

- `jit/planning.rs`
- `jit/validate.rs`
- `jit/layout.rs`
- `jit/emit_expr.rs`
- `jit/emit_term.rs`
- `jit/emit_calls.rs`
- `jit/cache.rs`
- `jit/runtime_symbols.rs`
- `jit/debug_render.rs`
- `jit/compile.rs`
- `jit/execute.rs`

Recommended split logic:

- planning and validation
- CLIF IR construction
- imported Python C-API/runtime symbols
- compiled artifact cache
- debug rendering / web export
- runtime entrypoints

Success condition:

- no single JIT file mixes plan interpretation, CLIF building, cache policy, and debug rendering.

## Phase 9. Simplify callable construction and wrapper metadata flow

Current function creation touches transformed source, `__dp__.py`, code-extra, and Rust bridge code.

Target model:

- transformed code emits a small structured function descriptor
- Rust/Python bridge materializes the final callable
- code-extra registration consumes that descriptor directly

Concrete steps:

- Define one descriptor shape for:
  - normal functions
  - generators
  - async generators
  - module init
- Make `def_fn`, `def_gen`, and `def_async_gen` thin adapters over that descriptor, or eliminate them if Rust can materialize the callable directly.
- Eliminate source-exec helper paths (`exec_function_def_source`) as structured metadata takes over.

Success condition:

- function creation behavior is owned in one place, not spread across helper conventions.

## Phase 10. Shrink `__dp__.py` after ownership is clarified

Do not split `__dp__.py` mechanically first. First remove responsibilities that should not live there.

After earlier phases, split it into clear surfaces:

- core semantic helpers
- scope/locals/globals helpers
- generator/coroutine runtime objects
- import/class helpers
- bootstrap/JIT registration helpers

Possible target layout:

- `__dp_core.py`
- `__dp_scope.py`
- `__dp_generators.py`
- `__dp_imports.py`
- `__dp_bootstrap.py`

If import-path simplicity matters more than module count, keep a single public `__dp__.py` that re-exports from those files.

Success condition:

- reading `__dp__.py` no longer means reading the entire runtime architecture at once.

## Phase 11. Clarify repository-level ownership

Concrete cleanup:

- Decide whether `soac-runtime/` is:
  - an active long-term crate
  - an experiment that should move under `experiments/`
- Remove or repurpose empty/dead top-level directories like `soac-codegen/`.
- Move perf artifacts and one-off notes out of repo root if they are not part of the product architecture.

Why:

- top-level ambiguity makes the repo feel larger and more confusing than it is.

## Phase 12. Rebuild testing around architectural layers

Current tests are strong on behavior but weak on stage-by-stage invariants.

Add distinct test layers:

1. AST rewrite tests
2. BB builder tests
3. BB validation tests
4. exception-lowering tests
5. CLIF planning tests
6. CLIF execution tests
7. integration/CPython behavior tests

Specific improvements:

- add fixture snapshots for canonical BB IR, not only rendered Python
- add tests for “unsupported shape” diagnostics at the planning boundary
- make web/debug viewers consume the same canonical BB/plan objects used at runtime

Success condition:

- refactors can be validated at the phase they change, instead of only via large behavior suites.

## Recommended Implementation Order

1. Add pipeline and BB validation infrastructure.
2. Move backend normalization into explicit BB passes.
3. Narrow BB IR and reduce round-tripping.
4. Move exception lowering to one owner.
5. Split `ast_to_bb` into true subphases.
6. Split `tree_walk` into Python-bridge vs legacy evaluator.
7. Remove `min_ast` from the main execution path.
8. Continue splitting JIT emit/execute/validate/cache/render.
9. Unify callable descriptor/materialization flow.
10. Shrink `__dp__.py`.
11. Clean up top-level repository leftovers.
12. Tighten tests around phase boundaries.

## What To Avoid

- Do not optimize for performance first by adding more special cases to already mixed-responsibility modules.
- Do not split files mechanically without clarifying ownership boundaries first.
- Do not keep both “semantic BB” and “backend-ready BB” implicit; make the stage transition explicit.
- Do not add new feature work directly into `jit/mod.rs`, `tree_walk/eval.rs`, `ast_to_bb/mod.rs`, or `__dp__.py` without first placing it in the right phase/module.

## Feature-Addition Guidance After The Refactor

Once the above structure exists, every new feature should be classifiable as one of:

- syntax normalization
- semantic lowering to BB
- exception/control-flow lowering
- backend-specific BB preparation
- runtime semantic helper
- Python bridge / callable materialization

If a new feature touches more than one of those categories, the interfaces are still too blurry.

## Concrete Near-Term Deliverables

These are the best next restructuring tasks to start with:

1. Add `transform/pipeline.rs` and `basic_block/validate.rs`.
2. Move `codegen_normalize.rs` into explicit BB passes and make plan registration require validated canonical BB.
3. Move exception lowering fully behind a single pass contract.
4. Split `tree_walk/eval.rs` into Python-bridge pieces before touching more execution behavior.
5. Remove `min_ast` from `soac-pyo3`’s primary execution path.

Those five changes will give the largest improvement in simplicity per unit of churn.
