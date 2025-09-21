from __future__ import annotations

import importlib
import sys
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from types import ModuleType

ROOT = Path(__file__).resolve().parent.parent
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

import diet_import_hook
import pytest

_MODULES_DIR = Path(__file__).resolve().parent / "integration_modules"


@contextmanager
def _load_integration_module(module_name: str) -> Iterator[ModuleType]:
    diet_import_hook.install()
    module_dir = str(_MODULES_DIR)
    module_path = _MODULES_DIR / f"{module_name}.py"
    if not module_path.exists():
        raise FileNotFoundError(
            f"Integration module '{module_name}' not found at {module_path}"
        )
    sys.path.insert(0, module_dir)
    try:
        sys.modules.pop(module_name, None)
        module = importlib.import_module(module_name)
        yield module
    finally:
        sys.modules.pop(module_name, None)
        if module_dir in sys.path:
            sys.path.remove(module_dir)


def pytest_configure(config):
    config.addinivalue_line("markers", "integration: mark a test as using integration modules")


@pytest.fixture
def run_integration_module():
    return _load_integration_module
