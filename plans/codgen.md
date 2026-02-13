# Cranelift Codegen Plan

## Goal
Generate machine code directly from the Basic Block (BB) structure using Cranelift, while keeping transform-mode Python rendering as a debug/compatibility view only.

This plan builds on `plans/bbstruct.md`: BB IR must be the canonical artifact; Python output is derived.

## Constraints
- Preserve existing Python-visible semantics (evaluation order, exceptions, closures, generators/async behavior).
- Do not add new minimal-AST variants.
- Keep transform-mode snapshots usable.
- Make runtime interfaces explicit and stable for JIT use.

## Target architecture

### 1) Canonical IR boundary
Introduce `BbModule`/`BbFunction`/`BbBlock` (from `bbstruct` plan) as the codegen input.

Codegen must never parse/render Python text. It consumes BB IR directly.

### 2) Two backends from the same IR
- `PyRenderBackend`: emits current `__dp__.def_fn/def_gen/...` style Python for transform mode/snapshots.
- `CraneliftBackend`: emits machine code stubs and metadata tables for eval/JIT execution.

### 3) Execution path split
- Transform mode: unchanged externally, still executes rendered Python.
- Eval/JIT mode: resolves function bodies to JIT entrypoints from BB IR.

## Cranelift lowering model

### Function-level lowering
Each `BbFunction` lowers to one Cranelift function with:
- `vmctx: pointer` (runtime context)
- `frame/state pointer` (locals/cells/generator state base)
- explicit function arguments (or packed args pointer for compatibility in phase 1)
- return: `{tag, payload}`-style terminator result for unified control flow

Preferred phase order:
1. Phase 1: preserve packed args/state ABI for easier parity.
2. Phase 2: switch to typed block params/locals ABI.

### Block-level lowering
Each `BbBlock` becomes a Cranelift block:
- block parameters are BB block params
- non-terminator ops become straight-line CLIF instructions/calls
- terminator maps to one CLIF terminator (`jump`, `brif`, `return`, trap/raise path)

### Terminator mapping
- `Jump(target, args)` -> `jump block_target(args...)`
- `BrIf(test, t, f)` -> `brif test, block_t(args...), block_f(args...)`
- `Ret(value)` -> encode terminal return tag/payload and `return`
- `Raise(exc)` -> encode raise tag/payload and `return` (or call trap helper)
- `Yield(pc,args,val)` -> encode yield tag/payload and `return`
- `TryJump` -> lowered via explicit exception dispatch protocol (see below)

### Value model
Initial representation: `PyObject*`-like pointer scalar in CLIF.
- integers/bools/None stay boxed like CPython objects (no immediate unboxing in phase 1).
- later optimization passes can unbox proven primitive paths.

## Exception and control-flow protocol

### Required simplification
Current `__dp__` uses Python tuple tags (`("jump", ...)`, etc.) and Python-level loop dispatch (`run_bb`, `try_jump_term`, thread-local exception override). This is good for prototyping but too dynamic for JIT.

Introduce a low-level runtime ABI:
- `DpTerm { tag: u8, a: ptr/int, b: ptr/int, c: ptr/int }`
- helpers return status + payload instead of raising directly where possible
- explicit exception slot in runtime context (`vmctx.current_exc`) instead of Python thread-local shims in hot path

`__dp__` keeps high-level wrappers for transform/debug mode, but JIT uses the low-level ABI directly.

### Try/except/finally handling
Lower `TryJump` with explicit dispatch blocks and runtime helpers:
- body execution returns `DpTerm`
- raise-path or helper error sets `vmctx.current_exc`
- except matcher helper (`exception_matches`) drives branch selection
- finally logic merges terms using deterministic merge rules (same as current `try_jump_term` contract)

No Python `try/except` should remain in JIT hot path.

## Generator / async / async generator

### Generator state layout
Replace dict-based state (`state["pc"]`, `state["args"]`, etc.) with fixed-layout runtime struct:
- `pc`
- args/locals slots
- pending send value
- pending throw exception
- done flag
- yield-from iterator slot(s)

Expose this layout in BB metadata so codegen and runtime agree.

### Dispatch
`br_table` lowers to CLIF switch-like dispatch on `pc`.
- no Python callable target lookup.
- targets are direct block labels or function pointers.

### send/throw/close
Generate direct JIT entrypoints for:
- `send(state, value)`
- `throw(state, exc)`
- `close(state)`

Keep Python `DpGenerator` wrapper as thin protocol adapter only.

### Async variants
Mirror above with awaitable protocol hooks, but same core state machine contract.

## Runtime simplification in `__dp__`

### Keep (compat layer)
- ergonomic helpers used by transformed Python in transform mode.
- fallback interpreter path for bring-up.

### Move out of hot path (to Rust runtime ABI)
- tuple-tag terminator protocol
- `run_bb`, `run_bb_term`, `run_coro_bb*`, `run_gen_bb*` loops
- thread-local exception/generator state coordination
- dynamic callable target dispatch for block jumps

### New runtime surface (Rust-first)
- `dp_exec_fn(vmctx, fn_id, frame_ptr, args...) -> DpTerm`
- `dp_exec_gen_send(vmctx, gen_state_ptr, value) -> DpTerm`
- `dp_exec_gen_throw(vmctx, gen_state_ptr, exc) -> DpTerm`
- reference-count-safe helper calls for attribute/call/binop/etc.

`__dp__.py` should call into these where needed for eval mode, but transform mode can keep Python fallback.

## Refcount and GC safety

### Ownership contract
Define every helper as one of:
- borrowed-in / borrowed-out
- borrowed-in / new-ref-out
- steals-ref

Encode this in codegen helper table to emit INCREF/DECREF correctly.

### Frame roots
All live `PyObject*` values in BB locals/params/state must be root-tracked until killed.
- maintain per-block liveness metadata.
- insert decref on dead edges (or central block-exit cleanup in phase 1).

### Exception objects
`vmctx.current_exc` must hold strong ownership until handled/cleared.

## Integration points

### `dp-transform`
- expose `transform_str_to_bb_ir_with_options`.
- preserve source maps and function metadata required by runtime (`name`, `qualname`, closures, param specs, generator tables).

### `soac-eval`
- add `codegen/` module using Cranelift frontend + module APIs.
- implement `BbModule -> CompiledModule` lowering.
- provide execution trampoline for Python `FunctionType` and generator wrappers.

### `soac-pyo3`
- eval path switches from `min_ast` execution to:
  1. transform to BB IR,
  2. compile/cache JIT artifact,
  3. install callable wrappers backed by JIT entrypoints.

## Incremental rollout

### Phase 0: Validator
- BB IR validator: label resolution, param arity, terminator correctness, dominance/liveness sanity.

### Phase 1: Cranelift skeleton backend
- Lower simple sync functions (`Jump/BrIf/Ret`) with helper calls.
- Keep Python fallback for unsupported ops.

### Phase 2: Exceptions + try/finally
- Implement `TryJump` lowering and term merge semantics.

### Phase 3: Generators
- Fixed generator state layout + `send/throw/close` entrypoints.

### Phase 4: Async / async generators
- Add await/async dispatch integration.

### Phase 5: Runtime cleanup
- Remove hot-path dependence on dynamic `__dp__` Python BB executor.
- Keep thin compatibility wrappers only.

## Testing strategy
- Differential tests: JIT vs current Python BB execution on same BB IR.
- Existing integration suites in both transform/eval modes.
- Dedicated refcount/exception lifecycle stress tests.
- Generator protocol tests (`next/send/throw/close`, `yield from`, async variants).
- Traceback/source-map parity checks.

## Deliverables
- `bb_ir` canonical types + validator.
- Cranelift backend crate/module with compile/cache APIs.
- Runtime ABI spec document (helper signatures + refcount contract).
- `__dp__` runtime simplification with clear fallback boundaries.
- Benchmarks showing dispatch overhead reduction vs Python BB interpreter loop.

## Immediate next step
Implement Phase 0 + Phase 1 for sync functions only:
1. BB IR validator,
2. Cranelift lowering for `Jump/BrIf/Ret`,
3. helper table + ownership annotations,
4. differential test harness against current `run_bb` behavior.
