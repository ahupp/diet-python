from __future__ import annotations

from pathlib import Path
import sys

import pytest
from collections.abc import Mapping, Iterator

import diet_import_hook
from tests._integration import split_integration_case


diet_python = diet_import_hook._get_pyo3_transform()


class _ModuleDictView(Mapping[str, object]):
    def __init__(self, module: object) -> None:
        self._module = module

    def __getitem__(self, key: str) -> object:
        return getattr(self._module, key)

    def __iter__(self) -> Iterator[str]:
        return iter(dir(self._module))

    def __len__(self) -> int:
        return len(list(dir(self._module)))

    def __contains__(self, key: object) -> bool:
        if not isinstance(key, str):
            return False
        return hasattr(self._module, key)


class _ModuleView:
    def __init__(self, module: object) -> None:
        self._module = module

    def __getattr__(self, name: str) -> object:
        return getattr(self._module, name)

    @property
    def __dict__(self) -> Mapping[str, object]:
        return _ModuleDictView(self._module)

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
def test_eval_source_simple_integration(case_path: Path, tmp_path: Path) -> None:
    source, validate_source = split_integration_case(case_path)
    module_name = case_path.stem
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text(source, encoding="utf-8")

    module = diet_python.eval_source(str(module_path))
    setattr(module, "__name__", module_name)

    view = _ModuleView(module)
    sys.modules[module_name] = view
    try:
        exec_globals = {"__name__": module_name}
        exec(compile(validate_source, str(case_path), "exec"), exec_globals)
    finally:
        sys.modules.pop(module_name, None)
