# REDESIGN v3

## Scope

This document reviews `dp-transform` from the public entrypoints down through AST rewriting, BlockPy lowering, and BB IR preparation. It focuses on:

- major components and their responsibilities
- notable data structures and what they represent
- end-to-end data flow
- overlapping responsibility and duplication
- refactorings that would improve legibility and maintainability
- high-level functions that should move closer to their only real consumers

The review is based on the current code in `dp-transform/`. It does not propose semantic changes to Python behavior.

## Executive Summary

The central maintainability problem is that `dp-transform` does not have one clearly authoritative mid-pipeline representation.

Today the public pipeline:

1. parses source into Ruff AST
2. rewrites the AST into a simplified transformed AST
3. emits a debug `BlockPyModule`
4. separately re-analyzes the same rewritten AST and lowers it again into `BbModule`

That means there are effectively two BlockPy-shaped pipelines:

- a debug/export path centered on `rewrite_ast_to_blockpy_module`
- an execution path centered on `rewrite_ast_to_lowered_blockpy_module`

The result is repeated scope analysis, repeated function identity derivation, repeated normalization, and repeated AST round-tripping between adjacent layers.

The second major issue is ownership blur:

- scope semantics are spread across `Context`, `ScopeTree`, name rewriting, and several helper analyses
- callable materialization is split across function identity, function lowering, annotation fallback, and export placement
- semantic lowering continues across AST rewrite, BlockPy lowering, BB construction, exception expansion, and codegen normalization

The code is careful about semantics and evaluation order, but the stage boundaries are hard to see, which makes the system harder to reason about and harder to extend safely.

## Current End-to-End Flow

At a high level, the public path is:

1. `transform_str_to_ruff_with_options` in `dp-transform/src/lib.rs`
2. `rewrite_module` in `dp-transform/src/driver.rs`
3. AST scope analysis and function identity collection
4. debug `BlockPyModule` emission via `basic_block::rewrite_ast_to_blockpy_module`
5. backend lowering via `basic_block::rewrite_ast_to_bb_module`
6. optional BB preparation for JIT or codegen via `prepare_bb_module_for_jit` and `prepare_bb_module_for_codegen`

Concretely:

- `lib.rs` parses source, builds `Context`, runs AST rewrites, then separately computes `BlockPyModule` and `BbModule`.
- `driver.rs` owns the AST-to-AST rewrite schedule.
- `basic_block/function_lowering.rs` walks rewritten functions and lowers execution-oriented function state.
- `basic_block/ruff_to_blockpy/mod.rs` lowers function bodies to BlockPy control-flow graphs.
- `basic_block/blockpy_to_bb/mod.rs` converts lowered BlockPy bundles into final `BbModule`.
- `basic_block/blockpy_to_bb/exception_pass.rs` and `codegen_normalize.rs` keep changing the BB form after initial construction.

That staging is directionally sensible, but the current module structure does not make the phase contracts explicit.

## Component Map

### 1. Public Orchestration

Primary files:

- `dp-transform/src/lib.rs`
- `dp-transform/src/driver.rs`
- `dp-transform/src/basic_block/mod.rs`

Notable data structures:

- `Options`
  - top-level transform configuration
- `TransformTimings`
  - parse/rewrite timing summary
- `LoweringResult`
  - bundle of transformed Ruff AST, optional debug `BlockPyModule`, and optional `BbModule`

Data flow:

- parse source into Ruff AST
- build `Context`
- run AST rewriting
- analyze module scope again
- collect function identity again
- lower to debug `BlockPyModule`
- lower to `BbModule`

Responsibilities:

- public API
- high-level phase ordering
- serialization to transformed source string

Current issues:

- orchestration is split between `lib.rs`, `driver.rs`, `basic_block/driver.rs`, and `web_inspector.rs`
- scope and identity are recomputed in multiple places
- `LoweringResult` exposes outputs from multiple logical stages, but the stages themselves are not first-class

### 2. Traversal and AST Construction Infrastructure

Primary files:

- `dp-transform/src/transformer.rs`
- `dp-transform/src/template.rs`
- `dp-transform/src/namegen.rs`

Notable data structures:

- `Transformer`
  - evaluation-order AST visitor used by almost every pass
- `SyntaxTemplate`
  - AST template parser/instantiator behind `py_stmt!` and `py_expr!`
- placeholder values and conversion traits
  - bridge from Rust values back into Ruff AST fragments

Data flow:

- passes walk Ruff AST via `Transformer`
- passes synthesize replacement AST via `py_stmt!`/`py_expr!`
- fresh helper/temp names come from `namegen`

Responsibilities:

- shared AST traversal contract
- AST fragment construction
- temporary name generation

Current issues:

- this layer is broadly good, but it is used so pervasively that duplication above it becomes easy to hide
- `namegen.rs` is minimal and global; the naming policy used in later BlockPy/BB lowering is partly separate from it

### 3. Scope and Context Analysis

Primary files:

- `dp-transform/src/basic_block/ast_to_ast/context.rs`
- `dp-transform/src/basic_block/ast_to_ast/scope.rs`
- `dp-transform/src/basic_block/ast_to_ast/scope_aware_transformer.rs`

Notable data structures:

- `Context`
  - rewrite-time context with source text, options, temp generation, and a lightweight scope stack
- `ScopeFrame`
  - transient rewrite-time scope state
- `ScopeTree`
  - persistent scope graph built from rewritten AST
- `Scope`
  - lexical scope node with bindings, local defs, and qualname information
- `BindingKind`
  - `Local`, `Nonlocal`, `Global`
- `ScopeKind`
  - `Function`, `Class`, `Module`

Data flow:

- `Context` is built before AST rewriting
- AST rewriting uses transient scope information
- after rewriting, `analyze_module_scope` builds a lexical scope tree over the rewritten AST
- downstream passes query `Scope` for binding and qualname decisions

Responsibilities:

- preserve Python binding semantics
- support explicit globals/nonlocals/class-body semantics
- derive function qualnames and binding targets

Current issues:

- there are effectively two scope systems:
  - transient rewrite-time `Context.scope_stack`
  - persistent post-rewrite `ScopeTree`
- several helper analyses partially duplicate scope-adjacent questions instead of routing through `Scope`

### 4. Rewrite Engine

Primary file:

- `dp-transform/src/basic_block/ast_to_ast/ast_rewrite.rs`

Notable data structures:

- `Rewrite`
  - statement rewrite result
- `LoweredExpr`
  - expression plus any prefix statements needed to preserve evaluation order
- `BodyBuilder`
  - helper for accumulating lowered expression side effects
- `StmtRewritePass`
- `ExprRewritePass`

Data flow:

- rewrite loop applies statement and expression passes to fixed point
- expression rewrites can emit statement prefixes
- function/class bodies are entered with explicit scope bookkeeping

Responsibilities:

- make expression-to-statement lowering safe
- centralize rewrite fixed-point logic

Current issues:

- this is the correct abstraction, but low-level adapters that belong here still live in higher-level modules
- notably `driver::SimplifyExprPass` is only an adapter onto `rewrite_expr::lower_expr`, but it lives in `driver.rs`

### 5. Front-End AST Rewrites

Primary files:

- `dp-transform/src/basic_block/ast_to_ast/rewrite_expr/mod.rs`
- `dp-transform/src/basic_block/ast_to_ast/rewrite_expr/comprehension.rs`
- `dp-transform/src/basic_block/ast_to_ast/rewrite_stmt/*`
- `dp-transform/src/basic_block/ast_to_ast/rewrite_class_def/*`
- `dp-transform/src/basic_block/ast_to_ast/rewrite_names.rs`
- `dp-transform/src/basic_block/ast_to_ast/rewrite_future_annotations.rs`
- `dp-transform/src/basic_block/ast_to_ast/rewrite_import.rs`
- `dp-transform/src/basic_block/ast_to_ast/simplify.rs`

Notable data structures:

- mostly pass-local rewriters rather than shared IR types
- `NameScopeRewriter`
  - converts implicit binding behavior into explicit runtime helper access

Data flow:

Current pass schedule in `driver.rs` is roughly:

1. rewrite future annotations
2. rewrite private class names
3. rewrite annotated assignments into `__annotate__` helpers
4. wrap module body inside `_dp_module_init`
5. run statement/expression simplification
6. analyze scope
7. rewrite globals/nonlocals/class-body binding behavior explicitly
8. rewrite class-body scopes
9. re-run simplification
10. strip generated placeholders

Responsibilities:

- lower rich Python syntax into a reduced transformed AST
- preserve evaluation order
- make implicit binding semantics explicit
- prepare the AST for BlockPy/BB lowering

Current issues:

- this layer is semantically dense and mostly coherent, but several concerns are duplicated:
  - type parameter lowering appears in both class and type-alias rewriting
  - comprehension and generator-expression lowering share similar machinery but live separately
  - string-literal escaping/encoding logic is repeated in several modules

### 6. Function Identity and Callable Materialization

Primary files:

- `dp-transform/src/basic_block/function_identity.rs`
- `dp-transform/src/basic_block/function_lowering.rs`
- `dp-transform/src/basic_block/annotation_export.rs`
- `dp-transform/src/basic_block/block_py/export.rs`

Notable data structures:

- `FunctionIdentity`
  - bind name, display name, qualname, binding target
- `FunctionIdentityByNode`
  - map from Ruff `NodeIndex` to runtime identity
- `BlockPyModuleRewriter`
  - AST visitor that intercepts `FunctionDef` and drives lowering/export
- placement/export plan enums in `block_py/export.rs`

Data flow:

- collect function identity from scoped AST
- during function lowering, decide whether a function is BB-lowerable
- if lowered:
  - lower function body to BlockPy and later BB
  - rewrite original `FunctionDef` into runtime binding statements
- if not lowered:
  - keep or rebuild the function from source text
  - possibly use annotation helper fallback machinery

Responsibilities:

- assign stable runtime identity
- decide function binding target
- package closure and annotation metadata
- rewrite `FunctionDef` into its transformed runtime form

Current issues:

- callable materialization is fragmented across four modules
- a simple behavior change around function export may require touching identity, lowering, annotation fallback, and export placement
- `FunctionIdentityByNode` is tuple-shaped, then converted back into `FunctionIdentity`, which weakens type clarity

### 7. BlockPy IR and Local Analyses

Primary files:

- `dp-transform/src/basic_block/block_py/mod.rs`
- `dp-transform/src/basic_block/block_py/dataflow.rs`
- `dp-transform/src/basic_block/block_py/state.rs`
- `dp-transform/src/basic_block/block_py/cfg.rs`
- `dp-transform/src/basic_block/block_py/exception.rs`
- `dp-transform/src/basic_block/block_py/pretty.rs`

Notable data structures:

- `BlockPyModule`
- `BlockPyFunction`
- `BlockPyBlock`
- `BlockPyStmt`
- `BlockPyExpr`
- `BlockPyGeneratorInfo`

What they represent:

- a structured control-flow IR that is lower than rewritten Ruff AST but still richer than final BB
- blocks still carry nested control constructs like `If`, `Try`, and `LegacyTryJump`
- generator metadata is attached here before final BB construction

Data flow:

- function bodies are lowered into `BlockPyFunction`
- CFG cleanup simplifies block shape
- dataflow computes block params and liveness-like state ordering
- exception helpers compute structured exception edges
- generator state helpers decide closure/cell synchronization

Responsibilities:

- explicit control-flow staging IR
- local CFG cleanup and analysis
- generator state modeling

Current issues:

- this layer is conceptually useful, but not clearly canonical
- some analyses still depend on converting BlockPy back into Ruff AST for convenience

### 8. Ruff AST to BlockPy Lowering

Primary files:

- `dp-transform/src/basic_block/ruff_to_blockpy/mod.rs`
- `dp-transform/src/basic_block/ruff_to_blockpy/generator_lowering.rs`

Notable data structures:

- `LoweredBlockPyFunction`
  - BlockPy function plus backend metadata
- `LoweredBlockPyFunctionBundle`
  - main function plus helper functions
- statement-sequence and generator lowering state structs

Data flow:

- lowered function input body enters sequence lowering
- loops, try/finally, with, break/continue, and generator constructs become explicit blocks
- generator metadata is synthesized
- block graph is finalized and annotated with:
  - block params
  - exception edges
  - closure layout
  - entry params
  - param specs

Responsibilities:

- heavy semantic lowering from simplified AST into BlockPy CFG
- generator and async generator lowering
- bridge from function semantics to backend metadata

Current issues:

- `ruff_to_blockpy/mod.rs` is the main monolith in this crate
- `LoweredBlockPyFunction` mixes:
  - semantic IR
  - analysis outputs
  - backend preparation state
- that makes ownership very hard to follow

### 9. BB IR and Backend Preparation

Primary files:

- `dp-transform/src/basic_block/bb_ir.rs`
- `dp-transform/src/basic_block/blockpy_to_bb/mod.rs`
- `dp-transform/src/basic_block/blockpy_to_bb/exception_pass.rs`
- `dp-transform/src/basic_block/blockpy_to_bb/codegen_normalize.rs`

Notable data structures:

- `BbModule`
- `BbFunction`
- `BbBlock`
- `BbOp`
- `BbExpr`
- `BbTerm`
- `BbGeneratorClosureLayout`

What they represent:

- final backend-facing CFG-like IR
- explicit blocks, ops, and terminators
- generator closure state layout

Data flow:

- lowered BlockPy bundles become `BbFunction`s
- block bodies are re-normalized during conversion
- exception edges are expanded from structured `TryJump`
- codegen normalization rewrites helper-call shapes into later backend-oriented forms

Responsibilities:

- final CFG IR construction
- explicit exception-edge lowering
- backend/codegen preparation

Current issues:

- semantic shaping continues after initial BB construction
- `BbExpr::from_expr` still rewrites attributes, subscripts, tuple construction, booleans, `None`, ellipsis, and strings
- `blockpy_to_bb` still converts through Ruff AST for some analysis and re-lowering

## Overlapping Responsibility and Duplication

### 1. Two BlockPy Pipelines

There are two AST-to-BlockPy-style paths:

- `rewrite_ast_to_blockpy_module`
  - debug/export view
- `rewrite_ast_to_lowered_blockpy_module`
  - execution-oriented lowering path

They operate on the same rewritten AST but produce different intermediate artifacts and duplicate some conceptual work.

### 2. Scope Questions Are Answered in Several Places

Scope semantics are spread across:

- `Context`
- `ScopeTree`
- `rewrite_names`
- `ast_symbol_analysis`
- `bound_names`
- `deleted_names`
- `block_py/state`

The problem is not that these helpers exist. The problem is that they are not clearly layered:

- some are lexical binding analyses
- some are flow analyses
- some are target/assignment analyses

but they all answer adjacent naming questions in partly overlapping ways.

### 3. Callable Materialization Is Split

Function creation logic spans:

- runtime identity derivation
- BB-lowerability decision
- annotation helper fallback
- lowered-function export planning
- source-based fallback export

This makes callable behavior one of the hardest parts of the system to place.

### 4. Simplification and Normalization Repeat

Normalization happens in multiple stages:

- AST rewriting
- function lowering
- BlockPy to BB conversion
- BB expression construction
- codegen normalization

This may be necessary in part, but the contracts are not explicit, so it feels like the same work keeps happening “one phase later”.

### 5. Repeated Helper Logic

Notable duplicated logic includes:

- type parameter lowering
- string literal byte escaping and decode-expression construction
- async/yield probing for generators and comprehensions
- temp/label/sanitization helpers
- parameter name collection

These are good candidates for shared submodules.

## Functions That Should Move Closer to Their Real Owner

### Top-level or high-level functions with narrow use

- `driver::SimplifyExprPass`
  - currently in `dp-transform/src/driver.rs`
  - real role is a low-level adapter over expression lowering
  - should move into `ast_rewrite` or `rewrite_expr`

- `class_def_to_create_class_fn`
  - currently in `dp-transform/src/basic_block/ast_to_ast/rewrite_class_def/mod.rs`
  - only relevant to class rewriting
  - should move into `class_body.rs` or a dedicated `class_factory.rs`

- `class_call_arguments`
  - same issue as above

- `strip_synthetic_module_init_qualname`
- `strip_synthetic_class_namespace_qualname`
  - currently in `ast_to_ast/util.rs`
  - effectively only part of function identity normalization
  - should move into `function_identity.rs`

- `blockpy_to_bb::push_lowered_blockpy_function_bundle`
  - only used from export logic
  - should become a method on `LoweredBlockPyModuleBundle` or move local to export

- `blockpy_to_bb::lower_blockpy_module_bundle_to_bb_module`
  - effectively only orchestration glue
  - should move into the bundle type or the driver/orchestrator

- `block_py::export::rewrite_function_def_stmt_via_blockpy`
  - only called from `BlockPyModuleRewriter`
  - should live next to `BlockPyModuleRewriter` or become a method on it

- `ruff_to_blockpy::lower_function_body_to_blockpy_function`
  - only called from `function_lowering`
  - should move under a `function_lowering/blockpy_builder.rs`-style module

- `ruff_to_blockpy::build_lowered_blockpy_function_bundle`
  - same issue as above

- `function_lowering::function_docstring_expr`
  - mostly consumed by export logic
  - should move next to function export/materialization

- `ruff_to_blockpy` compat helpers such as:
  - `compat_block_from_blockpy`
  - `compat_if_jump_block`
  - `compat_jump_block_from_blockpy`
  - `compat_raise_block_from_blockpy_raise`
  - `compat_return_block_from_expr`
  - these mostly support generator lowering and should live in a compat submodule near that code

## Recommended Refactoring Sequence

### 1. Introduce an Explicit Pipeline Module

Add a top-level pipeline/orchestration module with stage-owned types, for example:

- `ParsedModule`
- `RewrittenAst`
- `BlockPyDebugModule`
- `LoweredBlockPyBundle`
- `SemanticBbModule`
- `CodegenBbModule`

Benefits:

- one public orchestration path
- no more ad hoc rebuilds in `web_inspector.rs`
- easier to test stage contracts directly

### 2. Pick One Canonical Mid-Level Representation

Choose one:

- make `LoweredBlockPyModuleBundle` canonical and derive debug BlockPy from it
- or make `BlockPyModule` canonical and remove the second BlockPy path

Without this, the pipeline remains conceptually doubled.

### 3. Split `ruff_to_blockpy/mod.rs`

Break it into narrower submodules such as:

- `stmt_sequence.rs`
- `loops.rs`
- `try_lowering.rs`
- `with_lowering.rs`
- `generator_lowering.rs`
- `finalize.rs`
- `compat.rs`

This is the single biggest readability win inside the backend lowering layer.

### 4. Split `function_lowering.rs`

Break it into pieces by responsibility:

- module rewriter
- function support checker
- runtime body preparation
- identity and naming
- BlockPy builder glue

The current file mixes all of those concerns.

### 5. Consolidate Scope and Symbol Analysis

Create a small `analysis/` layer that clearly separates:

- lexical binding analysis
- assigned/bound name analysis
- deleted-name analysis
- BlockPy flow analysis

Then route common “what is this name?” questions through one authoritative service where possible.

### 6. Extract Shared Helper Submodules

High-value shared modules would be:

- `type_params.rs`
  - shared by class and type-alias lowering
- `string_literals.rs`
  - shared literal escaping and decode-expression construction
- `blockpy/analysis_adapter.rs`
  - BlockPy-to-Ruff adapters for analysis only
- `naming.rs`
  - temp/label/sanitize helpers

### 7. Separate Callable Materialization from IR Lowering

Group:

- `function_identity`
- `annotation_export`
- lowered/non-lowered export placement

into one focused area that owns:

- binding target decisions
- closure packaging
- annotation helper fallback
- transformed source replacements for functions

Then let `function_lowering` focus on deciding whether lowering is possible and on producing IR.

### 8. Unify Comprehension and Generator-Expression Lowering

The generator-expression path and comprehension path share too much shape-specific logic to remain separate at the current size.

Extract a shared lowering core for:

- async detection
- capture handling
- named-expression handling
- iteration skeleton generation

### 9. Make BB Phases Explicit by Contract

Define and name the BB stages more explicitly:

- semantic BB
- exception-expanded BB
- codegen-normalized BB

Then validate each stage boundary. Right now semantic shaping continues in:

- `BbExpr::from_expr`
- `blockpy_to_bb`
- `exception_pass`
- `codegen_normalize`

That makes it hard to know what the input contract of each phase really is.

## Suggested Near-Term Priorities

If this redesign work is staged, the highest-value sequence is:

1. create explicit pipeline orchestration and stage-owned types
2. choose one canonical BlockPy-stage representation
3. split `ruff_to_blockpy/mod.rs`
4. split `function_lowering.rs`
5. consolidate callable materialization ownership
6. extract duplicated helper modules

That sequence improves comprehension first, then shrinks the highest-entropy modules, then removes repeated local duplication.

## Summary

`dp-transform` already has the shape of a layered compiler pipeline:

- parse and rewrite Ruff AST
- lower to an explicit control-flow IR
- build backend BB IR
- normalize for later execution/codegen

The current problem is not that the architecture is fundamentally wrong. It is that stage boundaries and ownership are not yet reflected cleanly in the module structure.

The most important redesign move is to make the pipeline explicit and give one owner to each stage. Once that is done, many of the smaller duplications become straightforward cleanups instead of architectural guesswork.
