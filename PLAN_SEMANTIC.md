Plan: Migrate dp-transform scope analysis to ruff_python_semantic

Goal
- Replace dp-transform's custom scope analysis with a thin adapter over ruff_python_semantic's SemanticModel, preserving evaluation order and existing behavior.

Findings (current state)
- dp-transform uses a custom scope tree in dp-transform/src/transform/scope.rs and ScopeAwareTransformer in dp-transform/src/scope_aware_transformer.rs.
- ruff_python_semantic provides SemanticModel, Scopes, ScopeKind, and BindingKind (including Global/Nonlocal with target scope/binding IDs).
- ruff_python_semantic does not expose a standalone builder; Ruff's AST checker populates SemanticModel in vendor/ruff/crates/ruff_linter/src/checkers/ast/mod.rs.

Plan
1) Add dependency and new module
- Add ruff_python_semantic to dp-transform/Cargo.toml.
- Create dp-transform/src/transform/ruff_scope.rs (or similar) to own SemanticModel construction and adapter types.

2) Build a minimal SemanticModel builder (Transformer-based)
- Implement a Transformer that traverses StmtBody and updates SemanticModel:
  - push_scope/pop_scope for module, class, function, lambda, generator.
  - push_binding for assignments, imports, function/class defs, for/with/except targets, named expressions.
  - mirror Ruff's handling of global/nonlocal (copy small logic from Ruff checker).
- Construct Module for SemanticModel::new using ModuleKind::Module and ModuleSource::File(path). Use a real path when available; otherwise a placeholder path with .py to get correct PySourceType handling.
- Keep builder minimal to dp-transform needs; avoid full linting features.

3) Avoid TextRange identity for scope lookup
- Track a map from function/class nodes to ScopeId during traversal (prefer NodeIndex when present; fallback to pointer or address identity if needed).
- Use this map for child_scope_for_function/class in the adapter rather than SemanticModel::function_scope (range-based).

4) Adapter layer to preserve dp-transform APIs
- Provide a wrapper that exposes:
  - kind(): map Ruff ScopeKind to dp ScopeKind (Module/Class/Function). Treat Type/DunderClassCell/Generator/Lambda as transparent or map to Function where necessary.
  - scope_bindings(): name -> dp BindingKind (Global/Nonlocal/Local). Treat all non-Global/Nonlocal bindings as Local.
  - binding_in_scope, is_local/global/nonlocal using SemanticModel::lookup_symbol_in_scope and binding kind.
  - child_nonlocal_names via scan of all scopes for BindingKind::Nonlocal(_, scope_id).
- Preserve dp internal name rule: any name starting with _dp_ or __dp__ is treated as Local in the adapter.

5) Update call sites
- Replace analyze_module_scope with new adapter construction.
- Update dp-transform/src/transform/driver.rs and dp-transform/src/scope_aware_transformer.rs to use the adapter.
- Keep old implementation behind a feature flag if you want a quick fallback during migration.

6) Tests and parity
- Update existing scope tests in dp-transform/src/transform/scope.rs to use the new adapter.
- Add tests for:
  - nonlocal resolution across nested functions.
  - class-scope visibility of outer bindings.
  - comprehension/lambda boundaries if dp-transform relies on them.
- Run required tests: cargo test and ./scripts/pytest_cpython.sh tests/.
- If a fixture error occurs, run cargo run --bin regen_snapshots.

Risks / Notes
- Ruff includes extra scope kinds (Type, DunderClassCell, Generator/Lambda). Adapter must avoid behavior changes for dp-transform.
- SemanticModel::function_scope is range-based; avoid it to handle generated nodes with default ranges.
- QualNamer remains dp-transform responsibility (ruff_python_semantic does not provide qualname construction).

Done Criteria
- dp-transform produces identical transformed output for existing fixtures.
- Scope tests pass and no new CPython test failures are introduced.
