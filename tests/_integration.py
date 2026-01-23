from __future__ import annotations

import os
import importlib
import sys
from contextlib import contextmanager
from uuid import uuid4
from pathlib import Path
from types import ModuleType
from typing import Iterator

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import diet_import_hook

_VALIDATE_DELIMITER = "# diet-python: validate"


def split_integration_case(module_path: Path) -> tuple[str, str]:
    source = module_path.read_text(encoding="utf-8")
    if _VALIDATE_DELIMITER not in source:
        raise ValueError(f"missing integration validate delimiter in {module_path}")
    raw_source, raw_validate = source.split(_VALIDATE_DELIMITER, 1)
    return raw_source.rstrip() + "\n", raw_validate.lstrip("\n")


def _print_integration_failure_context(module_path: Path) -> None:
    try:
        source = module_path.read_text(encoding="utf-8")
    except OSError as err:
        source = f"<<failed to read source: {err}>>"

    try:
        transformed = diet_import_hook._transform_source(str(module_path))
    except Exception as err:
        transformed = f"<<failed to transform source: {err}>>"

    print("\n--- diet-python integration failure context ---", file=sys.stderr)
    print(f"module: {module_path}", file=sys.stderr)
    print("--- input module ---", file=sys.stderr)
    print(source, file=sys.stderr)
    print("--- transformed module ---", file=sys.stderr)
    print(transformed, file=sys.stderr)
    print("--- end diet-python integration context ---", file=sys.stderr)


@contextmanager
def transformed_module(
    tmp_path: Path, module_name: str, source: str
) -> Iterator[ModuleType]:
    package_name = f"_dp_integration_{uuid4().hex}"
    package_dir = tmp_path / package_name
    package_dir.mkdir(parents=True, exist_ok=True)
    (package_dir / "__init__.py").write_text("", encoding="utf-8")

    module_path = package_dir / f"{module_name}.py"
    module_path.write_text(source, encoding="utf-8")

    package_root = str(tmp_path)
    sys.path.insert(0, package_root)
    prior_allow_temp = os.environ.get("DIET_PYTHON_ALLOW_TEMP")
    os.environ["DIET_PYTHON_ALLOW_TEMP"] = "1"

    full_name = f"{package_name}.{module_name}"

    try:
        diet_import_hook.install()
        sys.modules.pop(full_name, None)
        sys.modules.pop(package_name, None)
        module = importlib.import_module(full_name)
        if prior_allow_temp is None:
            os.environ.pop("DIET_PYTHON_ALLOW_TEMP", None)
        else:
            os.environ["DIET_PYTHON_ALLOW_TEMP"] = prior_allow_temp
        yield module
    except Exception:
        _print_integration_failure_context(module_path)
        raise
    finally:
        sys.modules.pop(full_name, None)
        sys.modules.pop(package_name, None)
        if sys.path and sys.path[0] == package_root:
            sys.path.pop(0)
        else:
            try:
                sys.path.remove(package_root)
            except ValueError:
                pass
        if prior_allow_temp is None:
            os.environ.pop("DIET_PYTHON_ALLOW_TEMP", None)
        else:
            os.environ["DIET_PYTHON_ALLOW_TEMP"] = prior_allow_temp
