# REDESIGN v4

## Goals

This revision focuses on two related structural changes:

1. make `BlockPy` and `BbModule` stages over one shared CFG backbone instead of separate container shapes
2. make the boundary between `ast_to_ast` and `ast_to_blockpy` more explicit by pairing AST simplification and BlockPy lowering per statement family

The intent is not to collapse all stages into one semantic layer. The intent is to have one series of progressively more reduced CFG stages, with clear phase ownership and fewer parallel representations.

## 1. Shared CFG Backbone

The right end state is not a literal one-to-one merge of current `BlockPy` and current `BbModule`.

Those two stages still differ in meaningful ways:

- `BlockPy` still carries more Python-level structure
- `BB` has backend-oriented invariants such as block params, explicit exception edges, and reduced ops
- the terminator set differs by stage

What should be shared is the container shape.

### Proposed Shape

Use a generic CFG family that is parameterized by statement and terminator type, and optionally by expression and metadata types:

```rust
pub struct CfgModule<S, T, F = (), B = ()> {
    pub callable_defs: Vec<CfgCallableDef<S, T, F, B>>,
    pub module_init: Option<String>,
}

pub struct CfgCallableDef<S, T, F = (), B = ()> {
    pub function_id: FunctionId,
    pub bind_name: String,
    pub display_name: String,
    pub qualname: String,
    pub kind: CallableKind,
    pub params: Parameters,
    pub blocks: Vec<CfgBlock<S, T, B>>,
    pub meta: F,
}

pub struct CfgBlock<S, T, B = ()> {
    pub label: BlockLabel,
    pub body: Vec<S>,
    pub term: T,
    pub meta: B,
}
```

Then define stage aliases instead of separate top-level container families:

- `BlockPyModule<E, S = BlockPyStmt<E>, T = BlockPyTerm<E>>`
- `BbModule = CfgModule<BbOp, BbTerm, BbFunctionMeta, BbBlockMeta>`

### Important Consequence

If this direction is taken, terminators should not live in the stage statement type.

That means:

- no terminal ops embedded in the stmt enum
- the block `term` field is the only place control transfer lives
- a stage can still have structured terms such as `IfTerm`, `TryJump`, or `BranchTable`

That is the point where `BlockPy` and `BB` really become different specializations of one CFG family.

### Why This Is Better

This gives a single pass ladder:

1. parse + AST normalization
2. CFG with Python-structured statements and terms
3. CFG with reduced/core expressions and statements
4. CFG with backend-friendly ops and terms
5. codegen preparation

Instead of:

- one AST-to-BlockPy structure
- another BlockPy-to-BB structure
- several partly overlapping container types

The main benefit is that later passes become “reduce the stmt/term/meta types” rather than “translate into a mostly new graph representation again”.

## 2. Recommended Pass Ladder

With the shared CFG backbone, the pipeline should look more like this:

### AST-Normalization Passes

- scope analysis
- class lowering
- module wrapping
- function-definition extraction and instantiation rewriting
- explicit intrinsic-call rewriting for function construction

These passes still operate on Ruff AST and preserve Python source-level structure.

### AST to CFG(BlockPy, Ruff Expr)

This is the phase where all statement-level control flow becomes explicit blocks and terminators.

Examples:

- `if` becomes branch structure
- `while` / `for` become loop CFG
- `try` / `except` / `finally` become explicit handler/finally regions
- `with` / `async with` become explicit cleanup CFG

At this point expressions can still be Ruff-shaped if that keeps the transition manageable.

### CFG(BlockPy, Ruff Expr) to CFG(BlockPy, Core Expr)

This is expression reduction, not graph translation.

- intrinsic calls stay intrinsic calls
- expressions lose the full Ruff surface
- the stage becomes easier to interpret and easier to lower

### CFG(BlockPy/Core) to CFG(BB)

This is where backend-specific details appear:

- explicit block params where needed
- exception-edge metadata
- reduced backend ops
- any remaining SSA-oriented or codegen-oriented normalization

The result is still the same overall CFG family, just with more reduced statement/terminator/meta types.

## 3. Statement-Family Lowering Trait

The idea of pairing AST simplification and BlockPy lowering per statement family is good.

The main benefit is not the trait itself. The main benefit is that it forces each statement family to answer two explicit questions in one place:

1. is this syntax supposed to disappear entirely during `ast_to_ast`?
2. if not, what is its BlockPy lowering contract?

### Why This Helps

Right now the relationship between `ast_to_ast` and `ast_to_blockpy` is fairly unstructured.

Some constructs:

- are fully eliminated in AST rewriting
- are partly simplified in AST rewriting and then lowered again in BlockPy
- are handled by several modules that only indirectly encode the intended phase boundary

That makes it hard to tell whether a node arriving in `ast_to_blockpy` is expected or a bug.

### Base Idea

The user-proposed shape is directionally correct:

```rust
trait StmtLowerer<T> {
    fn simplify_ast(stmt: T) -> Stmt;

    fn to_blockpy(stmt: T) -> BlockPyBlockFragment {
        panic!("T should have already been reduced");
    }
}
```

The important part is the default `to_blockpy()` panic. That gives an automatic assertion that “if this node type was supposed to vanish earlier, it really did”.

### Recommended Refinement

The exact signatures above are too narrow.

`simplify_ast()` should not return a single `Stmt`, because many AST rewrites need to produce:

- zero statements
- multiple statements
- or a simplified residual node that still needs BlockPy lowering later

So the shape should be closer to:

```rust
trait StmtLowerer<T> {
    fn simplify_ast(
        &mut self,
        stmt: T,
        cx: &mut AstLowerCx,
    ) -> AstRewriteResult<T>;

    fn to_blockpy(
        &mut self,
        stmt: T,
        cx: &mut BlockPyLowerCx,
    ) -> BlockPyFragment {
        panic!("{} should have been eliminated by simplify_ast", type_name::<T>());
    }
}

enum AstRewriteResult<T> {
    Eliminated(Vec<Stmt>),
    Residual(T),
}
```

This keeps the useful assertion while still allowing the “simplify but do not erase” cases.

### Good Fit for This Model

This model works well for families like:

- `with` / `async with`
- `try` / `except` / `finally`
- `for` / `async for`
- `if`
- function-definition rewriting

For example:

- a simple annotation-only rewrite may return `Eliminated(...)`
- exception rewriting may return `Residual(T)` because it still expects explicit BlockPy exception regions later

## 4. What Should `to_blockpy()` Return?

This is the part that needs the most care.

A single `BlockPyBlock` is not enough.

Structured statements often need to create:

- several blocks
- entry jumps
- one or more continuation points
- loop exits
- exception-entry blocks
- cleanup/finally regions

So `to_blockpy()` needs to work in terms of spliceable fragments, not whole functions and not single blocks.

### Recommended Fragment Shape

A good fragment type should represent a control-flow slice that can be inserted into the enclosing function:

```rust
struct BlockPyFragment<S, T> {
    pub entry: FragmentEntry<S>,
    pub blocks: Vec<CfgBlock<S, T>>,
    pub normal_exit: FragmentExit,
}

enum FragmentEntry<S> {
    Inline(Vec<S>),
    Jump(BlockLabel),
}

enum FragmentExit {
    Open(BlockLabel),
    Closed,
}
```

This is enough for many cases:

- `Assign` lowers to `Inline([assign])`
- `If` lowers to an inline `IfTerm` plus owned branch blocks and an open fallthrough exit
- `Return` lowers to `Inline([])` plus a terminal `Return`

### Why Context Still Matters

For more complicated cases like exceptions, loops, and `finally`, the fragment alone should not carry all control state.

The lowering context should own the non-local control stacks:

```rust
struct BlockPyLowerCx {
    fresh_labels: ...,
    loop_stack: Vec<LoopTargets>,
    try_stack: Vec<TryTargets>,
    hoisted_defs: Vec<...>,
}
```

Then:

- the fragment describes the local CFG slice
- the context tracks surrounding control obligations

This is important because exception handling is not just another “open exit”. It needs coordination with the surrounding region stack.

## 5. Exception and `finally` Lowering Under This Model

This is the key stress case.

If `try` lowering returns only raw blocks with no context interaction, it becomes awkward to:

- identify which blocks are exception-entry blocks
- associate cleanup/finally regions with the correct protected body
- handle nested try regions cleanly

So the recommended model is:

- `to_blockpy()` for `try` allocates the local blocks and returns a fragment
- `BlockPyLowerCx` registers the exception-entry labels and surrounding try-region metadata
- enclosing lowering code only has to splice the fragment’s normal continuation

That keeps the fragment small while still letting exception semantics be explicit.

In other words:

- fragments describe structure local to the statement being lowered
- context describes obligations that outlive that one local structure

## 6. Intrinsics vs Enum Explosion

This design does not require turning every intrinsic into a dedicated giant Rust enum.

It is still fine, and likely preferable, to keep function instantiation and similar operations as compiler-owned intrinsic calls such as:

- `__dp_make_function(...)`
- `__dp_def_async_gen(...)`
- `__dp_def_coro_from_gen(...)`

The important distinction is:

- plan the semantics explicitly in Rust data first
- emit intrinsic calls only at the final expression/render boundary

That preserves the simplicity of intrinsic-call-based lowering while still making the phase structure explicit.

## 7. Practical Migration Order

The clean migration path is:

1. extract a shared `CfgModule` / `CfgCallableDef` / `CfgBlock` backbone
2. move `BlockPy` and `BB` onto that shared backbone as stage aliases
3. keep terminators out of the stage stmt types
4. introduce per-statement-family lowering traits for the major structured statements
5. make `ast_to_blockpy` use fragment-plus-context lowering instead of ad hoc control lowering helpers
6. progressively reduce the stage stmt/expr/term types without changing the container family again

## Summary

The main idea is:

- one CFG backbone
- many stage specializations
- one pass ladder that progressively reduces types

And the main AST-lowering idea is:

- pair AST simplification and BlockPy lowering by statement family
- make disappearance-vs-residual handling explicit
- use spliceable CFG fragments plus a lowering context for non-local control concerns

That would give a much clearer relationship between:

- `ast_to_ast`
- `ast_to_blockpy`
- `blockpy_to_bb`
- codegen preparation

while keeping the current intrinsic-call style rather than replacing it with a large intrinsic enum hierarchy.
