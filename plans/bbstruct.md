# BB Structure Plan

## Goal
Return a first-class Basic Block structure directly to JIT codegen, while keeping transform-mode Python output as a separate rendering concern.

## Why this is needed
Current BB lowering is still coupled to Python rendering:
- BB CFG is built in `dp-transform/src/transform/basic_block/mod.rs` as internal `Block`/`Terminator` using Ruff AST fragments.
- The same pass immediately renders BBs into Python function defs (`parse_function_skeleton`, `make_take_args_stmt`, `terminator_stmt`) plus `__dp__.def_fn/def_gen/def_async_gen` calls.
- Eval path then goes through transformed Python/min_ast instead of consuming CFG directly.

This coupling blocks direct JIT consumption and causes conversion fragility (for example, min_ast conversion failures from rendered helper shapes).

## Target architecture
Create two explicit artifacts from BB lowering:
1. `BbModule` (structural IR) for codegen/JIT.
2. `BbPyRender` (Python AST/source rendering) for transform-mode debugging, fixtures, and compatibility.

The BB structural IR is the canonical product. Python output is a derived view.

## Core design

### 1) Add a dedicated BB IR module
Create `dp-transform/src/bb_ir.rs` with stable, codegen-oriented data types.

Proposed top-level shape:
- `BbModule { functions, module_init, globals, source_map, options }`
- `BbFunction { id, name, qualname, kind, params, closure, binding_target, blocks, entry }`
- `BbBlock { id, label, params, ops, term }`
- `BbTerm` for terminators: `Jump`, `BrIf`, `Ret`, `Raise`, `Yield`, `TryJump`

Use existing expression/arg nodes where possible (reuse `min_ast::ExprNode`/`Arg`), or a minimal BB-op expression subset, but keep it separate from min_ast statement forms.

### 2) Split BB builder from renderer
Refactor `basic_block/mod.rs` into two layers:
- **Builder layer**: produces `BbFunction`/`BbBlock` graph and metadata only.
- **Renderer layer**: converts `BbModule` to current Python BB output (`__dp__.def_fn`, block defs, helper calls).

No JIT-facing logic should depend on emitted Python helper syntax.

### 3) Make transform APIs return BB IR
Extend `dp-transform` public API:
- Keep existing `transform_str_to_ruff_with_options` for compatibility.
- Add `transform_str_to_bb_ir_with_options` returning `BbModule`.
- Optionally include both artifacts in one `LoweringResult` to avoid duplicate transforms.

### 4) Route eval/codegen through BB IR
In `soac-pyo3`:
- Add a lowering path that asks `dp-transform` for `BbModule` directly.
- Stop requiring rendered Python as an intermediate for eval/JIT.

In `soac-eval`:
- Add BB consumer entrypoints (`eval_bb_module` now, JIT lowerer later).
- Keep current min_ast path behind fallback until parity is complete.

### 5) Preserve transform-mode output as a renderer only
Transform mode still outputs Python BB source for visibility/snapshots.
- `regen_snapshots` continues to use renderer.
- Snapshot stability comes from renderer parity tests against current output.

## Data contract details for JIT

### Function metadata contract
Each `BbFunction` must include:
- callable identity: `name`, `qualname`, `binding_target`
- signature data: ordered params with defaults/kinds
- closure capture contract: ordered captured bindings and cell/local distinctions
- kind enum: sync, coroutine, generator, async generator

### Control-flow contract
Each block must have:
- explicit parameter list (SSA-like incoming state tuple semantics)
- sequential non-terminator ops
- exactly one terminator
- explicit successor labels in terminator

### Generator/async contract
Represent generator state machine directly in BB IR metadata, not Python attributes:
- `start_pc`
- ordered target table
- throw-dispatch table
- resume-hook flags (or equivalent explicit dispatch edges)

Longer term: lower resume/throw dispatch fully into explicit CFG so side metadata shrinks.

### Exception-flow contract
`TryJump` stays explicit in BB IR with:
- body target
- except target
- optional finally target
- region label sets
- optional finally fallthrough target

This preserves current semantics while remaining JIT-translatable.

### Debug/source mapping
Include source location mapping on functions/blocks/ops/terms for traceback and diagnostics parity.

## Migration phases

### Phase 1: IR extraction without behavior change
- Introduce `bb_ir` types.
- Build IR in parallel with current rendering path.
- Add tests that IR and rendered output represent same CFG.

### Phase 2: Renderer consumes IR
- Remove direct CFG->Python emission from rewriter.
- Keep output identical by rendering from IR.
- Regenerate fixtures once parity is confirmed.

### Phase 3: Eval path consumes IR
- Add `soac-pyo3`/`soac-eval` path that executes IR directly.
- Keep min_ast fallback until regression set is green.

### Phase 4: JIT handoff API
- Define stable Rust interface for codegen crate to consume `BbModule`.
- Add translation smoke tests (IR -> mock backend / validator).

### Phase 5: Remove obsolete intermediate dependencies
- Reduce eval dependence on transformed Python text and min_ast for BB-lowered code.
- Keep Python renderer strictly for transform-mode output and debugging.

## Validation plan
- Unit: CFG invariants (`single terminator`, `all labels resolved`, `param arity checks`).
- Snapshot: renderer output parity against existing `snapshot_*.py` fixtures.
- Integration: transform/eval behavior parity suite.
- Differential: run current interpreter path vs IR path on same modules and compare outputs/exceptions.

## Risks and mitigations
- Risk: hidden Python-rendering assumptions leak into IR.
  - Mitigation: strict separation (`builder` has no `py_stmt!/py_expr!` usage).
- Risk: closure/cell semantics drift.
  - Mitigation: keep explicit closure schema and parity tests around `co_freevars/__closure__` behaviors.
- Risk: generator/try-flow semantics regress.
  - Mitigation: preserve existing `TryJump` and generator metadata first; simplify after parity.

## Concrete file-level changes
- Add `dp-transform/src/bb_ir.rs`.
- Split `dp-transform/src/transform/basic_block/mod.rs` into:
  - `builder` (CFG + IR creation)
  - `render_py` (IR -> Ruff AST/Python BB output)
- Extend `dp-transform/src/lib.rs` API to expose `BbModule` lowering.
- Update `soac-pyo3/src/eval.rs` to call BB IR API.
- Add IR consumer entry module in `soac-eval` (initial interpreter bridge, then JIT lowerer).

## Non-goals for this change
- No redesign of language semantics.
- No new minimal-AST variants.
- No forced removal of transform-mode Python output.
