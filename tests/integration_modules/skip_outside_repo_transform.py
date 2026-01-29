import importlib
import sys
from pathlib import Path


def imported_without_transform(tmp_path: Path) -> bool:
    module_name = "dp_outside_repo"
    module_path = tmp_path / f"{module_name}.py"
    module_path.write_text("VALUE = 1\n", encoding="utf-8")
    sys.path.insert(0, str(tmp_path))
    try:
        sys.modules.pop(module_name, None)
        module = importlib.import_module(module_name)
        return "__dp__" in module.__dict__
    finally:
        sys.modules.pop(module_name, None)
        if sys.path and sys.path[0] == str(tmp_path):
            sys.path.pop(0)

# diet-python: validate

module = __import__("sys").modules[__name__]
import tempfile
from pathlib import Path


with tempfile.TemporaryDirectory() as tmp_dir:
    tmp_path = Path(tmp_dir)
    assert module.imported_without_transform(tmp_path) is False
