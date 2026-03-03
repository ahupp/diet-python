from __future__ import annotations

import argparse
import importlib.util
import os
import sys
import types
from pathlib import Path

from . import _get_pyo3_transform


def _resolve_target(target: str) -> tuple[str, Path]:
    if os.sep in target or target.endswith(".py"):
        path = Path(target)
        if not path.is_file():
            raise SystemExit(f"diet_import_hook.exec: file not found: {target}")
        return path.stem, path.resolve()

    spec = importlib.util.find_spec(target)
    if spec is None or spec.origin is None:
        raise SystemExit(f"diet_import_hook.exec: module not found: {target}")
    if spec.origin in {"built-in", "frozen"}:
        raise SystemExit(
            f"diet_import_hook.exec: cannot execute built-in module: {target}"
        )
    return target, Path(spec.origin).resolve()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Execute a module via transformed source + module init"
    )
    parser.add_argument("module", help="Module name or path to a .py file")
    parser.add_argument("args", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)

    module_name, path = _resolve_target(args.module)
    run_name = "__main__"
    if path.name == "__init__.py":
        package = module_name
    else:
        package = module_name.rpartition(".")[0]
    package = package or None

    transform = _get_pyo3_transform()
    sys.argv = [str(path), *args.args]
    source = path.read_text(encoding="utf-8")
    transformed_source = transform.transform_source(source, True)
    module = types.ModuleType(run_name)
    module.__file__ = str(path)
    module.__name__ = run_name
    module.__package__ = package
    sys.modules[run_name] = module
    if module_name != run_name:
        sys.modules[module_name] = module
    sys.argv[0] = str(path)
    exec(compile(transformed_source, str(path), "exec"), module.__dict__)
    init = getattr(module, "_dp_module_init", None)
    if callable(init):
        init()
        try:
            delattr(module, "_dp_module_init")
        except Exception:
            pass

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
