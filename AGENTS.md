# AGENTS

## Design Goals

 * Prefer to make the codebase small.  This is not so much small in bytes on disk, but small in terms of what you need to know to navigate it.
   Ideally each independent concept should live in one place, and conversely each place should do discrete thing.

## Rules

- **MUST FOLLOW**: Always run `just test-all` before submitting changes, unless the change only updates project documentation such as `TODO.md`, `AGENTS.md`, or other docs-only files. `just test-all` runs `cargo test`, `just pytest tests/`, and `just build-web-inspector` in sequence after `just build-all`.
- **MUST FOLLOW**: Always preserve behavior in the transformed code, particularly evaluation order.
- **MUST FOLLOW**: When traversing the AST, always use an impl of `crate::transformer::Transformer`.
- **MUST FOLLOW**: When referring to a specific line or block of code, include both the full path with line number and the specific enclosing function, struct, trait, or other code item that contains it.
- **NOTE**: Prefer adding behavior at transform time rather than runtime in `__dp__.py` whenever possible.
- **MUST FOLLOW**: If a change requires adding a compatibility interface for a Python standard type/function, or patching one, stop and describe the reason before implementing.
- **MUST FOLLOW**: When changing implementation details, do not keep compatibility stubs/interfaces around; assume transformed inputs are regenerated each time.

## Tips
- **MUST FOLLOW**: If a fixture error occurs, regenerate all fixtures by running `cargo run --bin regen_snapshots` with no file arguments.
- **NOTE**: Use `cargo run --bin regen_snapshots` to regenerate fixtures instead of manual edits.
- **NOTE**: Check `snapshot/snapshot_summary.txt` after regenerating snapshots and flag any test case with a surprising BlockPy/CLIF block count, or any dramatic count change.
- **MUST FOLLOW**: Keep `snapshot_*` updates in the current logical change instead of restoring them away; include real snapshot output changes in the same change so BlockPy/rendering regressions stay immediately visible.
- **NOTE**: Set `DIET_PYTHON_INTEGRATION_ONLY=1` to only transform integration test modules (skip transforming all imports).
- To inspect the transformed output of some code, run `cargo run --bin diet-python file_with_code.py`, which prints output to stdout.
- *MUST FOLLOW* when fixing a bug that fails a cpython test case *always* add a minimal reproducing integration test to reproduce it first.
- CPython source for tests is vendored at `vendor/cpython` (the scripts use `vendor/cpython/python`).
- **MUST FOLLOW**: When running Python directly in this repo, always use `vendor/cpython/python` unless the user explicitly requests a different interpreter.
- **NOTE**: For `just run-cpython-tests 0 -f <file>`, pass an absolute path for `<file>` since regrtest runs from `vendor/cpython`.
- **NOTE**: In sandboxed environments, set `--tempdir /tmp/<dir>` when running CPython tests; default worker temp dirs under `/home/adam/project/cpython/build/...` can fail with permission errors.
- **NOTE**: After interrupting CPython test runs, clean stale workers before retrying (`pkill -f test.libregrtest.worker`).
- **NOTE**: For sequential shard runs, use `./scripts/run_cpython_test_sets.sh`; it enforces single-process regrtest via `just run-cpython-tests 1`, JIT execution, absolute set paths, and a safe tempdir.
- **NOTE**: For hangs under the transformed runtime, use `vendor/cpython/python` (or `.venv/bin/python`) with `faulthandler.dump_traceback_later(..., exit=True)` to capture a Python stack before terminating.
- **MUST FOLLOW**: When you find a hang, add follow-up instrumentation where practical so the next diagnosis is easier, and add a focused regression test or assertion for the diagnosed hang shape instead of treating it as a one-off.
- **NOTE**: For isolated transformed-runtime repros, prefer `tests._integration.transformed_module(...)` with a small inline source module instead of debugging through the full test harness.
- **NOTE**: For BB/JIT inspection, use `diet_import_hook._get_pyo3_transform().jit_has_bb_plan(...)` / `jit_render_bb_with_cfg_plan(...)`; closure-backed outer factories are typically registered under `qualname::_dp_bb_<name>_factory`.
- **NOTE**: To trace BB execution, set `DIET_PYTHON_BB_TRACE`. Accepted forms are `all`, `all:params`, `<exact-qualname>`, or `<exact-qualname>:params`. Prefer an exact qualname (for example `make_runner.<locals>.run:params`) to keep trace output manageable.
- **MUST FOLLOW**: In any test failure summary, list expected failures separately from unexpected failures.
- When running tests, put the output in logs/
- **MUST FOLLOW**: If a new PR is requested, open a new jj change first with `jj new`, then immediately update its description so the head (`@`) is up to date using `jj describe -m <message> @`, including both the change summary and the rationale.
- **MUST FOLLOW**: If a new PR is requested, first make a concrete implementation plan for the requested change, include that plan in the jj head (`@`) description, and unless the user explicitly requests no confirmation, share the plan and get confirmation before implementing.
- **MUST FOLLOW**: For each logical change, update the top commit description with `jj describe -m "<message>" @`, then create a new commit with `jj new` before starting the next logical change.
- **MUST FOLLOW**: After each logical change, run `jj diff --stat` and show a concise summary of the size and location of the change.
- **MUST FOLLOW**: When completing one step in a multi-stage plan, explain the next concrete step. If stopping instead of continuing, explicitly say the current line is done and then describe the next suggested plan.
- **MUST FOLLOW**: If a user request starts with `TODO`, add it to the `## Codex TODO Intake` section of `TODO.md`.
- **MUST FOLLOW**: If a response to a `TODO` request includes a plan or other useful information, include that in the corresponding `TODO.md` entry.
- **MUST FOLLOW**: When a TODO is completed, move it from `## Codex TODO Intake` to `## Completed` in `TODO.md` and include a brief description of the work done.
- **MUST FOLLOW**: At the end of each completed response for a `TODO` request, list the TODOs one per line and include a summary of the last response.
- **MUST FOLLOW**: When a `jj describe` message needs multiple paragraphs or sections, pass actual newlines, not literal `\n`. Use shell multiline quoting, for example:
  `jj describe -m "$(cat <<'EOF'
  Summary line

  Rationale:
  - first point
  - second point
  EOF
  )" @`
