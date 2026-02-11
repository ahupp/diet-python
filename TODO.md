
## BB Lowering: Unsupported / Not Fully Lowered

- Function-level exclusions:
- `async def` is excluded from BB lowering.
- Function docstrings exclude the function from BB lowering.
- Function signature/return annotations exclude the function from BB lowering.
- Generated annotation helpers (`__annotate__`, `__annotate_func__`) are excluded.
- Empty function bodies are excluded.
- Eval mode currently skips BB lowering.

- Generator exclusions:
- Any generator containing `yield from` is excluded.
- Any generator containing `await` is excluded.
- Generated genexprs that load outer `_dp_cell_*` / `_dp_classcell` are excluded.
- Non-genexpr generators must satisfy `is_simple_generator_function_body`:
- only `pass`, `assign`, `function def`, `yield` expr statements, and `return`.
- no `if/while/for/try/with/match`, etc.

- Statement/expression exclusions in non-generator BB support checker:
- `async for` unsupported.
- `try*` / `except*` unsupported.
- `await`, `yield`, `yield from` in expression traversal unsupported for non-generator BB path.
- `break` / `continue` outside loops unsupported.
- Any surviving unsupported stmt kind (for example `class`, `with`, `match`) causes fallback.

- Try lowering that is not fully BB-split (`try_jump`) today:
- `try` with `else` and/or `finally`.
- Handler shapes beyond plain dispatch candidate shape.
- Nested `try` in the lowered `try` region.
- `try` containing `break` / `continue`.
- Cases with defs in the remaining stmt slice.

- Known CFG/liveness follow-up:
- `del` is modeled via sentinel rewrite, but BB liveness still needs kill-set modeling.

## Best Next Step

- Implement full `try/except/else/finally` BB terminator lowering (single structured path, no linear `try` fallback), then expand generator support to `yield from` on top of that control-flow substrate.
 
