# REDESIGN

## Objectives
Prioritize changes in this order:
1. Improve understandability.
2. Simplify for reliability and future changes.
3. Improve performance.

Constraints carried forward:
- Preserve Python semantics and evaluation order.
- Keep AST traversal via `Transformer`-style visitors.
- Avoid new minimal-AST variants unless explicitly requested.

## Current Structure (High-Level)

### Workspace responsibilities
- `dp-transform/`: Python->Python lowering pipeline + basic-block (BB) lowering + BB IR generation.
- `soac-pyo3/`: Python extension bridge that invokes lowering and registers BB plans for eval/JIT.
- `soac-eval/`: eval-frame runtime and Cranelift JIT path consuming BB plans.
- `__dp__.py`: compatibility/runtime helper layer for transformed code and BB function wrappers.
- `tests/`: large integration/regression suite (including CPython-focused behavior tests).

### End-to-end flow today
1. Parse source via Ruff parser.
2. Run multi-pass AST rewriting in `rewrite_module`.
3. Rewrite names/scopes to explicit runtime helper calls (`__dp_*`).
4. Optionally lower to BB and collect `BbModule`.
5. Render transformed Python and execute via import hook / eval path.
6. Runtime functions in `__dp__.py` dispatch to BB/JIT behavior.

This works, but the implementation is spread across several very large mixed-responsibility files.

## Current Structure (Low-Level Findings)

### 1) Monolithic modules with mixed responsibilities
- `dp-transform/src/basic_block/ast_to_bb/mod.rs` (~5459 LOC): CFG building, generator state machine lowering, exception lowering, liveness, label rewriting, function metadata, annotation helper handling, and tests in one file.
- `soac-eval/src/jit/mod.rs` (~5056 LOC): planning, IR shape selection, codegen, runtime-call emission, and many optimization special-cases in one file.
- `__dp__.py` (~75k LOC): core helper semantics, eval/exec shims, BB wrappers, generator/coroutine runtime, JIT dispatch glue.

Understandability impact: high cognitive load and unclear ownership boundaries.

### 2) Repeated logic in multiple places
Representative duplication:
- Type-param construction duplicated:
  - `rewrite_class_def::make_type_param_info`
  - `rewrite_stmt::type_alias::make_type_param_info`
- Comprehension async detection and named-expression target collection duplicated between:
  - `rewrite_expr/mod.rs`
  - `rewrite_expr/comprehension.rs`
- Name/assignment/load collectors duplicated across:
  - `rewrite_names.rs`
  - `ast_to_bb/mod.rs`
  - `scope.rs`
  - `min_ast.rs`
- Byte-string escaping helpers duplicated across multiple modules (`rewrite_expr`, `string`, `simplify`, `bb_ir`, `codegen_normalize`).

Reliability impact: fixes diverge and semantics can drift subtly.

### 3) Early concretization of name binding semantics
`rewrite_names::rewrite_explicit_bindings` rewrites many loads/stores to concrete runtime helpers before BB lowering. This forces later phases to recover semantic intent from already-concretized helper calls.

Observed downstream complexity:
- BB lowering and normalization carry many special-cases for names/cells/deleted-name handling.
- Harder to reason about whether evaluation order and exception behavior are preserved uniformly.

### 4) Custom scope and CFG/liveness logic is extensive and hand-rolled
- `transform/scope.rs` implements custom scope tree and binding analysis.
- `ast_to_bb/mod.rs` implements separate use/def/liveness and block-param computation.

This duplicates concepts already present in vendored Ruff semantic infrastructure and increases maintenance burden.

### 5) String-source re-exec path for function definitions
There are source-string execution paths (for annotation helpers and non-BB def fallback) via `__dp_exec_function_def_source(...)` and `exec_function_def_source(...)`.

Reliability impact:
- Harder traceback/source-map consistency.
- More moving parts around closure capture and helper binding.

## Redesign Proposals (Prioritized)

## A. Improve Understandability First

### A1) Introduce an explicit lowering pipeline spec
Create a `Pipeline` declaration (ordered phases + contracts), rather than implicit sequencing in `rewrite_module`.

Each phase should declare:
- Input invariants.
- Output invariants.
- Whether it can run fixpoint-style.
- Which language features it is responsible for eliminating.

Immediate benefit: pass ordering and responsibilities become legible and reviewable.

### A2) Split `ast_to_bb` into focused modules with strict ownership
Refactor `basic_block/ast_to_bb/mod.rs` into submodules, e.g.:
- `builder/control_flow.rs` (statement->block graph lowering)
- `builder/generator.rs` (yield/yield-from/async-generator state machine lowering)
- `builder/exception.rs` (try/except/finally lowering)
- `analysis/liveness.rs` (state vars, block params)
- `analysis/names.rs` (load/def/kill extraction)
- `naming/labels.rs` (label generation/renaming)
- `metadata/function_identity.rs`

Keep one `mod.rs` as orchestration only.

### A3) Add a dedicated BB visitor/rewriter abstraction
Introduce `BbTransformer` (parallel to AST `Transformer`) for `BbModule` traversal.

Suggested trait surface:
- `visit_module`
- `visit_function`
- `visit_block`
- `visit_op`
- `visit_term`
- `visit_expr`

Use it in:
- `codegen_normalize`
- `soac-eval/jit/exception_pass`
- future BB analyses and CLIF preparation passes

Answer to your question: yes, this abstraction should be introduced.

### A4) Create shared transform utility modules for duplicated operations
Centralize duplicated helpers into reusable modules:
- `transform/common/names.rs`: assigned/load/bound/parameter collectors.
- `transform/common/type_params.rs`: one `make_type_param_info` implementation.
- `transform/common/literal_bytes.rs`: one escape/decode helper set.
- `transform/common/comprehension_async.rs`: async detection and named-expr target utilities.

This is low-risk and immediately improves readability and consistency.

### A5) Split runtime helper layers in `__dp__.py`
Restructure into logical modules (can still re-export same public names):
- `runtime_core.py`: operator/name/class/exception helpers.
- `runtime_scope.py`: locals/globals/cell/frame proxy logic.
- `runtime_bb.py`: BB wrappers and generator/coroutine object adapters.
- `runtime_jit.py`: JIT plan dispatch and CLIF-wrapper glue.

This makes runtime responsibilities inspectable and lowers review friction.

### A6) Define a first-class CLIF planning boundary
Today CLIF/JIT behavior is spread between BB planning, CLIF rendering, and runtime metadata plumbing.

Proposal:
- Introduce a `ClifPlan` IR derived from `BbFunction` (explicitly modeling only CLIF-lowerable shapes).
- Split CLIF pipeline into three explicit stages:
  1. `BbFunction -> ClifPlan` validation/planning.
  2. `ClifPlan -> CLIF text` (debug output).
  3. `ClifPlan -> compiled code` (execution path).
- Make both `jit_render_*` and `jit_run_*` consume the same `ClifPlan` object to prevent drift.

Answer to your question: yes, defining CLIF should be a dedicated structured layer, not an emergent side-effect of mixed JIT code paths.

## B. Simplify for Reliability and Future Changes

### B1) Delay global/cell/class binding concretization
Current behavior concretizes binding operations too early.

Proposal:
- Preserve symbolic binding intent through most transforms and BB lowering.
- Store resolved binding mode in IR metadata (local/global/nonlocal-cell/class-lookup).
- Perform final concretization at backend boundary (Python renderer or JIT lowering).

Why:
- Keeps semantic intent explicit longer.
- Reduces helper-call pattern matching later.
- Improves confidence in evaluation-order preservation.

Answer to your question: making cells/globals concrete this early is likely not the right long-term structure.

### B2) Leverage Ruff scope analysis (first), then CFG/use-def selectively
Use vendored Ruff semantic crates as primary scope authority where practical.

Pragmatic adoption path:
1. Add a differential checker in tests: compare current `analyze_module_scope` results with Ruff-derived scope/binding classification on representative modules.
2. Once parity is good, migrate production scope resolution to Ruff-backed data.
3. Keep local adapter types to avoid leaking Ruff internals everywhere.

This targets correctness for class scopes, nonlocal/global edge cases, and future Python syntax drift.

Answer to your question: yes, scope analysis should more heavily leverage Ruff.

### B3) Use Ruff CFG/use-def incrementally, not as a big-bang replacement
Ruff CFG in this tree is simpler than this project’s exception/generator semantics. Full replacement is risky.

Recommended compromise:
- Use Ruff CFG/use-def for straightforward regions first (non-generator/non-try-heavy blocks).
- Keep custom advanced lowering where semantics exceed Ruff CFG model.
- Add differential liveness checks in tests to prevent regressions.

### B4) Replace source-string function re-exec with structured lowering objects
Eliminate (or sharply reduce) `exec_function_def_source` paths by carrying structured function-def payloads and closure captures through IR/backend APIs.

Benefits:
- Better traceback/source coherence.
- Less fragile capture/default-parameter synthesis.
- Fewer parse/render/exec loops.

### B5) Strengthen IR validation gates
Before registration/execution, validate `BbModule` invariants centrally:
- all labels resolved
- exactly one terminator per block
- block param arity consistency on all edges
- no unsupported expression forms at backend boundary
- explicit exception-edge ownership

Fail early with actionable diagnostics.

## C. Performance Improvements (after A/B)

### C1) Remove avoidable AST round-trips
Current passes often do `Expr -> string/template parse -> Expr` or `BbExpr -> Expr -> BbExpr`.

After structural cleanup:
- use direct AST constructors where possible
- reduce parse-expression/template churn in hot paths

### C2) Reduce whole-tree fixpoint cost
`rewrite_with_pass` repeatedly traverses until stable.

Possible follow-up:
- pass-local worklists
- phase-local fixed-point boundaries
- dirty-subtree tracking

### C3) Simplify BB normalization and JIT fast-path planning
Once BB passes are modularized and validated:
- avoid repeated shape inference passes over same blocks
- cache immutable plan artifacts by `(module, qualname, bb-version)`

### C4) Improve state threading precision
Current block-param computation can over-thread state.

After liveness refactor:
- include explicit kill-set semantics (noted TODO exists)
- reduce parameter traffic and runtime frame writes

## Answers to Requested Design Questions

### Should we introduce abstractions for common operations?
Yes.
- Introduce `BbTransformer` for BB traversal.
- Introduce shared `transform/common/*` helpers for repeated collectors and literal/type-param logic.
- Keep AST-level traversal on `Transformer` as required.

### Should we more heavily leverage Ruff (CFG and scope analysis)?
Yes, with sequencing:
- Scope analysis: adopt first (high payoff, manageable migration).
- CFG/use-def: adopt selectively and incrementally, because this project’s exception/generator semantics are richer than Ruff’s basic CFG layer.

### Is making cells/global concrete so early a good idea?
No.
- Delay concretization to backend lowering.
- Carry binding intent explicitly in IR metadata through earlier phases.

## Proposed Module Layout (Target)

`dp-transform/src/transform/`
- `pipeline.rs`
- `phase_*.rs`
- `common/names.rs`
- `common/type_params.rs`
- `common/literal_bytes.rs`

`dp-transform/src/basic_block/`
- `ir/` (data types + validator)
- `builder/control_flow.rs`
- `builder/generator.rs`
- `builder/exception.rs`
- `analysis/liveness.rs`
- `analysis/names.rs`
- `passes/normalize_codegen.rs`
- `passes/label_cleanup.rs`
- `visitor.rs` (`BbTransformer`)

`soac-eval/src/`
- `jit/clif_plan.rs`
- `jit/planner.rs`
- `jit/lower_expr.rs`
- `jit/lower_term.rs`
- `jit/runtime_calls.rs`
- `jit/exception_edges.rs`

`runtime` (Python)
- `__dp__/core.py`
- `__dp__/scope.py`
- `__dp__/bb.py`
- `__dp__/jit.py`

## Migration Plan

### Phase 0: Baseline + guardrails
- Keep behavior locked with existing tests.
- Add differential checks for scope/liveness equivalence where new logic is introduced.

### Phase 1: Readability extraction
- Split monolith files.
- Add `BbTransformer` and shared helpers.
- No semantic changes.

### Phase 2: Reliability refactors
- Move to delayed binding concretization.
- Introduce IR validation.
- Migrate scope analysis to Ruff-backed adapter.

### Phase 3: Runtime cleanup
- Reduce source-string re-exec paths.
- Split `__dp__` runtime modules while preserving API surface.

### Phase 4: Performance passes
- remove AST round-trips
- improve fixpoint/worklist behavior
- tighten liveness and state threading

## Expected Outcome
- Easier to understand ownership and behavior of each phase.
- Lower semantic-regression risk for future Python feature work.
- Cleaner handoff to CLIF/JIT codegen and fewer ad-hoc conversions.
- Better performance opportunities unlocked by clearer IR boundaries.
