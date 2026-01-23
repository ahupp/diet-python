from __future__ import annotations

from contextlib import contextmanager
from pathlib import Path
import sys

import pytest

from tests._integration import split_integration_case, transformed_module

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
def test_integration_case(tmp_path: Path, case_path: Path) -> None:
    source, validate_source = split_integration_case(case_path)
    module_name = case_path.stem

    sys.path.insert(0, str(MODULES_DIR))
    try:
        with transformed_module(tmp_path, module_name, source) as module:
            @contextmanager
            def run_integration_module(name: str):
                if name != module_name:
                    raise AssertionError(
                        f"unexpected module name {name!r} (expected {module_name!r})"
                    )
                yield module

            namespace: dict[str, object] = {
                "__name__": f"tests.integration_validate.{module_name}",
                "__package__": "tests",
                "__file__": str(case_path),
                "run_integration_module": run_integration_module,
            }
            exec(validate_source, namespace)
            validate = namespace.get("validate")
            if validate is None:
                raise AssertionError(f"validate(module) not defined in {case_path}")
            validate(module)
    finally:
        if str(MODULES_DIR) in sys.path:
            sys.path.remove(str(MODULES_DIR))
