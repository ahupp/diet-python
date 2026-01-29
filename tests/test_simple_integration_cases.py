from __future__ import annotations

from pathlib import Path
import sys

import pytest

from tests._integration import (
    exec_integration_validation,
    split_integration_case,
    transformed_module,
    untransformed_module,
)

MODULES_DIR = Path(__file__).resolve().parent / "simple"


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
@pytest.mark.parametrize("transform", [False, True], ids=["untransformed", "transformed"])
def test_simple_integration_case(tmp_path: Path, case_path: Path, transform: bool) -> None:
    source, validate_source = split_integration_case(case_path)
    module_name = case_path.stem
    module_loader = transformed_module if transform else untransformed_module

    sys.path.insert(0, str(MODULES_DIR))
    try:
        with module_loader(tmp_path, module_name, source) as module:
            exec_integration_validation(
                validate_source, module, case_path, transformed=transform
            )
    finally:
        if str(MODULES_DIR) in sys.path:
            sys.path.remove(str(MODULES_DIR))
