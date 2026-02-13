# Annotation Plan

## Goal
Build one annotation pipeline that is:
- mode-consistent (`transform` and `eval` behave the same),
- compatible with Python 3.15 `annotationlib`/`inspect`/`pydoc` expectations,
- mostly transform-time (minimal runtime patching),
- aligned with BB-first lowering (no separate non-BB annotation behavior).

## Current Problems
Today annotations are split across multiple mechanisms:
- Module/class annassign rewrite emits `__annotate__` / `__annotate_func__` helpers.
- Function annotations are handled separately via `__dp__.apply_fn_metadata(...)` and eager `__annotations__` assignment.
- Eval mode also has a separate Rust-side `soac_function_annotate_pyfunc` / `eval_function_annotations` path.
- Runtime `annotationlib` patching is still present in `__dp__.py` for non-eval.

This creates drift between modes and repeated edge-case fixes.

## Target Design (Single Source of Truth)
Use a single transform-time annotation model for **module, class, and function** owners.

### 1) Canonical annotation payload at transform time
For every annotation entry, capture:
- key (field/param/`return`),
- expression AST (existing min-ast `ExprNode`, no new AST variants),
- original source string (for `STRING` behavior),
- owner/scope capture metadata needed to evaluate correctly.

### 2) One generated annotate callable shape
For every owner, generate an annotation callable with CPython-compatible semantics:
- module/function: `__annotate__(format, /, ...)`
- class: store `__annotate_func__` in class dict (type machinery exposes callable `__annotate__`)

Supported format behavior:
- `VALUE`: evaluate expressions in real scope.
- `VALUE_WITH_FAKE_GLOBALS`: evaluate with provided fake globals, preserving closure/nonlocal bindings.
- `STRING`: return original annotation source strings.
- `FORWARDREF`: either direct forwardref construction or `NotImplementedError` fallback behavior matching CPython+annotationlib expectations (pick one and enforce via tests).

### 3) Function annotations: stop being special
Replace separate function-only metadata behavior with the same annotation callable model used by module/class.
- Keep `__doc__` assignment as metadata.
- Derive `__annotations__` from `__annotate__(VALUE)` consistently.
- Keep ordering/evaluation semantics compatible with CPython.

### 4) Eval mode uses the same model
Eliminate eval-only annotation implementation differences:
- remove `soac_function_annotate_pyfunc` / `eval_function_annotations` path,
- execute the same transformed annotation helper logic in eval mode.

This removes mode-specific annotation behavior and makes failures reproducible from transformed source.

### 5) Remove annotationlib monkey patches
After unified behavior passes tests, delete:
- `_patch_annotationlib`,
- `_ensure_annotationlib_import_hook`,
- `_annotationlib_patch_enabled`,
and related import-hook wiring.

If any patch must remain, document a narrow reason and add a TODO with owning test.

## Implementation Plan

### Phase 0: Lock behavior with tests
Add/normalize cross-mode tests for:
- module/class/function annotations,
- `annotationlib.get_annotations` for all formats,
- closure/nonlocal/classcell capture in annotations,
- fake-globals behavior,
- `inspect`/`pydoc` visibility (no accidental internal helper leakage),
- `from __future__ import annotations` parity.

Run in both `transform` and `eval`.

### Phase 1: Introduce unified annotation builder at transform time
- Build a shared transform utility that emits annotate helpers for all owner kinds.
- Migrate function annotation path from `apply_fn_metadata` tuples/lambdas to this utility.
- Preserve current external behavior while both old/new paths temporarily coexist behind one internal switch.

### Phase 2: Switch eval mode to transformed annotation helpers
- Route eval function/class/module objects to the same emitted helpers.
- Remove Rust annotation evaluation entrypoints after tests pass.

### Phase 3: Remove runtime patching
- Delete annotationlib patch/import-hook code in `__dp__.py`.
- Keep only generic runtime helpers that are not annotationlib-specific.

### Phase 4: Cleanup + simplify
- Remove obsolete annotation compatibility branches.
- Keep one code path for annotation generation/execution.
- Update docs/tests to state supported semantics explicitly.

## Key Invariants
- No new minimal-AST variants.
- Annotation semantics must be mode-independent.
- Evaluation order must remain CPython-compatible.
- BB lowering must not require a separate annotation fallback path.

## Risks and Mitigations
- **Risk:** closure/nonlocal names rewritten in transformed scopes break annotation evaluation.
  - **Mitigation:** carry explicit scope-capture metadata for annotation helpers and test with nested class/function/nonlocal cases.

- **Risk:** fake-globals semantics differ subtly from annotationlib expectations.
  - **Mitigation:** test directly against `annotationlib.call_annotate_function` and `get_annotations` format matrix.

- **Risk:** helper names leak into user-facing APIs (`pydoc`, module `__all__`, class dict details).
  - **Mitigation:** explicit integration tests + naming/filtering policy for generated internals.

## Acceptance Criteria
- Same results in `transform` and `eval` for annotation-focused integration tests.
- No annotationlib import-hook/patch code remains.
- Function/class/module annotations use one generation path.
- Existing annotation regressions pass without mode-specific xfails.
