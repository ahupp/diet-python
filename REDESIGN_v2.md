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

The repository has converged on a basic-block-centric architecture, but it still carries a large amount of bridge code between:

- Ruff AST rewriting
- BB IR generation
- BB normalization for codegen
- CLIF planning/codegen
- Python runtime helper construction in `__dp__.py`
- vectorcall/Python-bridge plumbing in `soac-eval`

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
  - Python vectorcall bridge integration.
  - CLIF planning and JIT execution.
  - Main files: `soac-eval/src/tree_walk/eval.rs`, `soac-eval/src/jit/mod.rs`, `soac-eval/src/jit/planning.rs`, `soac-eval/src/jit/exception_pass.rs`.

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
8. Those helpers in `__dp__.py` create placeholder Python functions/generator wrappers and register lazy CLIF vectorcall metadata.
9. On first function call, the vectorcall bridge in `soac-eval/src/tree_walk/eval.rs` compiles the CLIF entry if needed and dispatches directly into the JIT path.
10. `soac-eval/src/jit/*` executes the CLIF-backed plan.

### Removed legacy execution path

The old `min_ast` + Rust tree-walk interpreter path has been removed from normal execution.

The remaining architectural issue is no longer “two live execution engines”. It is:

- some stale `min_ast`-related surfaces still exist in the repository shape
- BB pipeline stages are still implicit
- exception lowering is still structurally split across transform and JIT-facing code
- Python callable materialization remains spread across transformed source, `__dp__.py`, and the Rust bridge

### BB IR / planning path today

1. AST lowering emits `BbModule`.
2. `dp-transform/src/basic_block/codegen_normalize.rs` mutates BB IR again into a codegen-oriented shape.
3. `soac-eval/src/jit/planning.rs` lowers `BbModule` to `ClifPlan`.
4. `soac-eval/src/jit/exception_pass.rs` rewrites `TryJump` into explicit exception edges before planning.
5. `soac-eval/src/jit/mod.rs` turns `ClifPlan` into Cranelift IR and executable code.

This is directionally correct, but the BB stage boundaries are still not explicit enough in the type/module structure.

## Architectural Problems

## P1. BB stage boundaries are still implicit

The repository now has one live execution architecture, but the stages inside it are still blurrier than they should be:

- semantic lowering to BB
- BB normalization
- structured exception lowering
- backend-ready BB preparation
- CLIF planning / codegen

That makes it harder than necessary to answer where a new feature belongs.

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

## P3. Exception lowering responsibility is split, but should remain two-phase

Today:

- AST-to-BB lowering emits structured try control flow (`TryJump`) intended to model CLIF-like `try_call` / `try_jump` semantics
- `exception_pass` lowers that further into explicit exception edges and plain control flow
- planning/codegen consume the lowered form

The split itself is correct. The problem is that the boundary is not explicit enough in the IR model or module ownership.

## P4. Callable materialization is spread across Python and Rust

Function creation today touches:

- transformed Python source
- `_dp_module_init`
- `__dp__.py` helper constructors
- vectorcall registration
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
- `tree_walk/eval.rs` combines vectorcall interception, lazy compilation, callable materialization, and Python bridge/runtime glue.
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
   - `SemanticBbModule -> LoweredBbModule`
   - `LoweredBbModule -> ClifPlan`
   - `ClifPlan -> CLIF`
   - `ClifPlan -> compiled machine code`

4. Python callable materialization is driven by a small structured descriptor, not by source re-exec or broad helper pattern matching.

5. `soac-eval/tree_walk` is reduced to Python-interpreter integration glue:
   - vectorcall bridge
   - lazy compilation/caching
   - callable materialization
   - Python runtime glue
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

Status: mostly complete.

Concrete steps:

- Keep `min_ast` out of the live execution path.
- Remove or isolate any remaining `min_ast`-shaped public/workspace surfaces that still imply it is part of the runtime architecture.
- Keep transformed module execution dependent on BB IR registration and callable metadata only.

Current shape:

- `LoweringResult` returns transformed Ruff AST string + canonical `BbModule`.
- `soac-pyo3` registers plans directly from `BbModule`.
- `tree_walk` keeps vectorcall bridge/runtime plumbing only.

Success condition:

- All normal transformed execution goes through `BbModule`.
- `min_ast` no longer shapes runtime-facing design decisions.

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

## Phase 5. Make the two exception-lowering phases explicit and typed

Exception handling should be represented in two explicit forms only:

1. structured exception regions in semantic BB (`TryJump`)
2. explicit exception edges/handlers in lowered backend-ready BB

Concrete steps:

- Introduce an explicit semantic/lowered BB distinction in naming and validation.
- Keep AST-to-BB lowering focused on semantic control-flow structure and CLIF-like `TryJump`.
- Keep the second phase as a separate pass that lowers `TryJump` into explicit exception edges and plain `brif`/`jump` structure.
- Move or rename that second-phase pass so it is clearly a BB-lowering phase, not an accidental JIT-only detail.
- The JIT should consume lowered backend-ready exception edges, not own semantic exception lowering.

Specific representation improvements:

- Represent exception edges as typed metadata on blocks/terminators rather than stringly `exc_target_label` / `exc_name` pairs.
- Add separate validators for:
  - semantic BB that may contain `TryJump`
  - lowered BB that may not contain `TryJump`

Success condition:

- The two exception phases are obvious in the pipeline and type/module structure.
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

## Phase 7. Split `soac-eval/tree_walk` into “Python bridge” pieces

The current name `tree_walk` is misleading because it now primarily holds Python runtime/vectorcall bridge logic rather than tree walking.

Target structure:

- `soac-eval/src/py_bridge/vectorcall.rs`
- `soac-eval/src/py_bridge/materialize.rs`
- `soac-eval/src/py_bridge/compile_cache.rs`
- `soac-eval/src/py_bridge/function_metadata.rs`
- `soac-eval/src/py_bridge/runtime_glue.rs`

Concrete steps:

- Move vectorcall interception and lazy compilation concerns under a “Python bridge” name.
- Move callable materialization and metadata handling out of the same file as low-level runtime glue.

Success condition:

- `tree_walk/eval.rs` no longer exists as a large mixed-responsibility file.

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

Current function creation touches transformed source, `__dp__.py`, vectorcall registration, and Rust bridge code.

Target model:

- transformed code emits a small structured function descriptor
- Rust/Python bridge materializes the final callable
- vectorcall registration consumes that descriptor directly

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
2. Make semantic BB vs lowered BB explicit.
3. Move backend normalization into explicit BB passes.
4. Narrow BB IR and reduce round-tripping.
5. Make the second exception-lowering phase an explicit BB pass contract.
6. Split `ast_to_bb` into true subphases.
7. Split `tree_walk` into Python-bridge pieces.
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
2. Make semantic BB vs lowered BB an explicit contract.
3. Move `codegen_normalize.rs` into explicit BB passes and make plan registration require validated canonical BB.
4. Make the second exception-lowering phase an explicit BB pass with clear input/output invariants.
5. Split `tree_walk/eval.rs` into Python-bridge pieces before touching more execution behavior.

Those five changes will give the largest improvement in simplicity per unit of churn.
