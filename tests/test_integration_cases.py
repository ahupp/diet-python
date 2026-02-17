from __future__ import annotations

from pathlib import Path
import sys

import pytest

from tests._integration import (
    exec_integration_validation,
    integration_module,
    split_integration_case,
)

MODULES_DIR = Path(__file__).resolve().parent / "integration_modules"


def _case_paths() -> list[Path]:
    cases: list[Path] = []
    for path in sorted(MODULES_DIR.glob("*.py")):
        try:
            if "# diet-python: validate" in path.read_text(encoding="utf-8"):
                cases.append(path)
        except OSError:
            continue
    return cases


@pytest.mark.integration
@pytest.mark.parametrize("case_path", _case_paths(), ids=lambda path: path.stem)
@pytest.mark.parametrize(
    "mode",
    ["stock", "transform", "eval"],
    ids=["stock", "transformed", "eval"],
)
def test_integration_case(tmp_path: Path, case_path: Path, mode: str) -> None:
    if case_path.stem == "yield_from_stack_names" and mode == "transform":
        # BB-lowered generators do not preserve CPython frame-name identity for
        # sys._getframe() observations yet.
        pytest.xfail("BB generator frame-name observability not yet CPython-compatible")
    if mode == "eval" and case_path.stem in {"locals_cell_contents", "scope_locals"}:
        pytest.xfail("eval-mode locals() normalization for closure/cell vars is not yet aligned")
    if mode == "eval" and case_path.stem in {
        "async_contextmanager_stopiter",
        "generator_exception_context",
        "yield_from_module",
    }:
        pytest.xfail("eval-mode generator/with exception flow is currently unstable")
    if mode in {"transform", "eval"} and case_path.stem in {
        "exception_refcycle_after_except",
        "exception_refcycle_args_tuple",
        "taskgroup_propagate_cancellation_refcycle",
        "asyncio_taskgroup_base_error_refcycle",
    }:
        # Dict-backed frame locals are currently GC-visible, unlike CPython's
        # fast-locals representation. This changes gc.get_referrers() behavior
        # for local exception objects in these refcycle-sensitive cases.
        pytest.xfail("frame-local dict storage is GC-visible in BB transform path")

    source, validate_source = split_integration_case(case_path)
    module_name = case_path.stem

    sys.path.insert(0, str(MODULES_DIR))
    try:
        with integration_module(tmp_path, module_name, source, mode=mode) as module:
            exec_integration_validation(validate_source, module, case_path, mode=mode)
    finally:
        if str(MODULES_DIR) in sys.path:
            sys.path.remove(str(MODULES_DIR))
