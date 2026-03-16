# Function Types Through Lowering

This note records the function-representation types used by the lowering pipeline, in order, and what each one adds or removes.

## Shared chassis

Most later function types are wrappers around [`CfgCallableDef`](../dp-transform/src/basic_block/cfg_ir.rs):

- `function_id`
- `bind_name`
- `display_name`
- `qualname`
- `kind`
- `params`
- `entry_liveins`
- `blocks`

The main differences between stages are:

- the block / stmt / term / expr representation stored in `blocks`
- how much function metadata lives outside the `CfgCallableDef`
- whether one source function has already been expanded into helper callables

## Pipeline

### 1. Ruff AST

Type:

- `ast::StmtFunctionDef`

Used in:

- `ast_to_ast` rewrites
- scope rewriting
- helper-function synthesis

Properties:

- still Python-shaped
- rich statements and expressions
- sync vs async is carried by `is_async: bool`
- no CFG yet

Relevant files:

- [`dp-transform/src/basic_block/function_lowering.rs`](../dp-transform/src/basic_block/function_lowering.rs)
- [`dp-transform/src/basic_block/ast_to_ast/rewrite_stmt/function_def.rs`](../dp-transform/src/basic_block/ast_to_ast/rewrite_stmt/function_def.rs)

### 2. Semantic BlockPy callable

Types:

- `BlockPyCallableDef<E, B>`
- `SemanticBlockPyCallableDef = BlockPyCallableDef<Expr>`

Defined in:

- [`dp-transform/src/basic_block/block_py/mod.rs`](../dp-transform/src/basic_block/block_py/mod.rs)

Properties:

- first CFG-shaped function form
- blocks contain semantic BlockPy statements / terms / expressions
- semantic expressions are just Ruff `Expr`, and are still rich:
  - `If`
  - `Compare`
  - `Await`
  - `FString`
  - `TString`
  - etc.
- adds function metadata outside the bare CFG:
  - `doc`
  - `closure_layout`
  - `local_cell_slots`

### 3. Internal semantic staging

These are internal `ruff_to_blockpy` transport types:

- `PreparedBlockPyFunction`
- `LoweredBlockPyFunctionBundle`

Defined in:

- [`dp-transform/src/basic_block/ruff_to_blockpy/mod.rs`](../dp-transform/src/basic_block/ruff_to_blockpy/mod.rs)

What they are for:

- `PreparedBlockPyFunction`
  - semantic callable plus generator / try-region planning metadata
  - fields:
    - `callable_def`
    - `generator_metadata`
    - `try_regions`
- `LoweredBlockPyFunctionBundle`
  - one source function may lower into:
    - one main callable
    - zero or more helper callables
  - used for resume helpers, closure-backed generator helpers, etc.

These are staging types, not long-lived pipeline boundary types.

### 4. Lowered semantic transport

Type:

- `LoweredBlockPyFunction`

Defined in:

- [`dp-transform/src/basic_block/ruff_to_blockpy/mod.rs`](../dp-transform/src/basic_block/ruff_to_blockpy/mod.rs)

Properties:

- wraps `SemanticBlockPyCallableDef`
- carries metadata needed by later BlockPy / BB passes:
  - `bb_kind`
  - `block_params`
  - `exception_edges`
  - `runtime_closure_layout`

At the module level:

- `LoweredBlockPyModuleBundle = CfgModule<LoweredBlockPyFunction>`

Defined in:

- [`dp-transform/src/basic_block/blockpy_to_bb/mod.rs`](../dp-transform/src/basic_block/blockpy_to_bb/mod.rs)

This is the tracked `semantic_blockpy` pass result.

### 5. Core BlockPy callable

Types:

- `CoreBlockPyCallableDef = BlockPyCallableDef<CoreBlockPyExpr>`
- `CoreBlockPyExpr`

Defined in:

- [`dp-transform/src/basic_block/block_py/mod.rs`](../dp-transform/src/basic_block/block_py/mod.rs)

Properties:

- same CFG/function wrapper shape as semantic BlockPy
- expressions have been reduced to the core surface:
  - `Name`
  - `Literal`
  - `Call`
  - `Await`
  - `Yield`
  - `YieldFrom`
- structured BlockPy control flow still exists here
- f-strings / t-strings are gone by this point

### 6. Lowered core transport

Type:

- `LoweredCoreBlockPyFunction`

Defined in:

- [`dp-transform/src/basic_block/blockpy_to_bb/mod.rs`](../dp-transform/src/basic_block/blockpy_to_bb/mod.rs)

Properties:

- wraps `CoreBlockPyCallableDef`
- keeps the same side metadata pattern as lowered semantic transport:
  - `bb_kind`
  - `block_params`
  - `exception_edges`
  - `runtime_closure_layout`

At the module level:

- `LoweredCoreBlockPyModuleBundle = CfgModule<LoweredCoreBlockPyFunction>`

This is the tracked `core_blockpy` pass result.

### 7. Final BB function

Type:

- `BbFunction`

Defined in:

- [`dp-transform/src/basic_block/bb_ir.rs`](../dp-transform/src/basic_block/bb_ir.rs)

Properties:

- wraps `CfgCallableDef<FunctionId, LoweredFunctionKind, Vec<String>, BbBlock>`
- params are flattened to name lists instead of Ruff `Parameters`
- blocks are BB blocks, not BlockPy blocks
- statements are the final no-await/no-yield core BlockPy stmt type
- terms are BB-specific
- still carries side metadata:
  - `binding_target`
  - `closure_layout`
  - `local_cell_slots`

`LoweredFunctionKind` is also more backend-specific than `BlockPyFunctionKind`; generator kinds carry explicit resume metadata.

## Short version

The main pipeline boundary types are:

1. `ast::StmtFunctionDef`
2. `SemanticBlockPyCallableDef`
3. `LoweredBlockPyFunction`
4. `CoreBlockPyCallableDef`
5. `LoweredCoreBlockPyFunction`
6. `BbFunction`

The internal extras are:

- `PreparedBlockPyFunction`
- `LoweredBlockPyFunctionBundle`

## Cleanup direction

The main cleanup opportunity is not to delete all of these blindly, but to clarify which are:

- true phase-boundary types
- internal staging types
- generic wrappers that exist only because metadata has not yet been normalized into a shared place

If this area is simplified later, the goal should be:

- fewer “transport” wrappers
- more explicit phase boundaries
- fewer places where helper-function expansion and per-function metadata are both being threaded separately
