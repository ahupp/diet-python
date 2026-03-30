from __future__ import annotations

import argparse
import builtins
import importlib.machinery
import importlib.util
import os
import sys
import tempfile
import types
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
_PYO3_TRANSFORM = None


def _integration_only_enabled() -> bool:
    # Read dynamically so tests can toggle this per import context.
    return os.environ.get("DIET_PYTHON_INTEGRATION_ONLY") == "1"


def _run_module_init(module) -> None:
    init = getattr(module, "_dp_module_init", None)
    if init is None:
        return
    try:
        init()
    finally:
        try:
            delattr(module, "_dp_module_init")
        except Exception:
            pass


def _build_module_init(path: str, module):
    try:
        with open(path, "r", encoding="utf-8") as file:
            original_source = file.read()
    except OSError as err:
        raise ImportError(f"diet-python could not read source for {path}: {err}") from err
    transformer = _get_pyo3_transform()
    try:
        init = transformer.build_module_init(
            original_source,
            module.__dict__,
        )
    except SyntaxError as err:
        if err.filename is None:
            err.filename = path
        raise
    except Exception as err:
        raise ImportError(f"diet-python failed for {path}: {err}") from err
    return init


def _get_pyo3_transform():
    global _PYO3_TRANSFORM
    if _PYO3_TRANSFORM is None:
        try:
            import diet_python as transform
        except Exception as err:
            raise ImportError(
                "diet-python extension is required but could not be imported; "
                "run 'just build-all' or 'just build-extension <debug|release>'"
            ) from err
        _PYO3_TRANSFORM = transform
    return _PYO3_TRANSFORM


def _is_integration_module(resolved: Path) -> bool:
    try:
        resolved.relative_to(REPO_ROOT / "tests" / "integration_modules")
        return True
    except (OSError, ValueError):
        pass
    for parent in resolved.parents:
        if parent.name.startswith("_dp_integration_"):
            return True
    return False


def _should_transform(path: str) -> bool:
    """Return ``True`` if ``path`` should be passed through the transform."""
    if path.endswith(os.path.join("encodings", "__init__.py")):
        return False
    try:
        resolved = Path(path).resolve()
    except OSError:
        return False
    if resolved.name == "__init__.py" and resolved.parent.name == "encodings":
        return False
    if resolved.name == "templatelib.py" and resolved.parent.name == "string":
        return False
    if _integration_only_enabled() and not _is_integration_module(resolved):
        return False
    if os.environ.get("DIET_PYTHON_ALLOW_TEMP") != "1":
        try:
            resolved.relative_to(Path(tempfile.gettempdir()).resolve())
        except (OSError, ValueError):
            pass
        else:
            return False
    try:
        with open(path, "rb") as file:
            return b"diet-python: disable" not in file.read()
    except OSError:
        return False


class DietPythonLoader(importlib.machinery.SourceFileLoader):
    """Loader that applies the diet-python transform before executing a module."""

    def create_module(self, spec):
        return None

    def exec_module(self, module):
        module.__dict__.setdefault("__builtins__", builtins.__dict__)
        init = _build_module_init(self.path, module)
        if init is not None:
            module._dp_module_init = init
        _run_module_init(module)
        return None


class DietPythonFinder(importlib.machinery.PathFinder):
    """Finder that wraps loaders to apply diet-python transformations."""

    @classmethod
    def find_spec(cls, fullname, path=None, target=None):
        spec = super().find_spec(fullname, path, target)
        if fullname == "encodings" or fullname.startswith("encodings."):
            return spec
        if (
            spec
            and isinstance(spec.loader, importlib.machinery.SourceFileLoader)
            and spec.origin
            and _should_transform(spec.origin)
        ):
            spec.loader = DietPythonLoader(fullname, spec.origin)
        return spec


def install():
    """Install the diet-python import hook."""
    # Ensure the transform module is loaded before we intercept stdlib imports.
    _get_pyo3_transform()
    try:
        from . import runtime as runtime_module

        hook_fn = getattr(runtime_module, "_ensure_annotationlib_import_hook", None)
        if hook_fn is not None:
            hook_fn()
        transform = _get_pyo3_transform()
        runtime_module._jit_make_bb_function = getattr(transform, "make_bb_function", None)
        runtime_module._jit_make_bb_generator = getattr(transform, "make_bb_generator", None)
    except Exception:
        pass

    existing_typing = sys.modules.get("typing")
    if existing_typing is not None:
        loader = getattr(getattr(existing_typing, "__spec__", None), "loader", None)
        if not isinstance(loader, DietPythonLoader):
            sys.modules.pop("typing", None)

    if any(finder is DietPythonFinder for finder in sys.meta_path):
        return

    for index, finder in enumerate(sys.meta_path):
        if finder is importlib.machinery.PathFinder:
            sys.meta_path.insert(index, DietPythonFinder)
            break
    else:
        sys.meta_path.insert(0, DietPythonFinder)


def _resolve_target(target: str) -> tuple[str, Path]:
    if os.sep in target or target.endswith(".py"):
        path = Path(target)
        if not path.is_file():
            raise SystemExit(f"soac.import_hook: file not found: {target}")
        return path.stem, path.resolve()

    spec = importlib.util.find_spec(target)
    if spec is None or spec.origin is None:
        raise SystemExit(f"soac.import_hook: module not found: {target}")
    if spec.origin in {"built-in", "frozen"}:
        raise SystemExit(f"soac.import_hook: cannot execute built-in module: {target}")
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

    install()
    transform = _get_pyo3_transform()
    sys.argv = [str(path), *args.args]
    source = path.read_text(encoding="utf-8")
    module = types.ModuleType(run_name)
    module.__file__ = str(path)
    module.__name__ = run_name
    module.__package__ = package
    sys.modules[run_name] = module
    if module_name != run_name:
        sys.modules[module_name] = module
    sys.argv[0] = str(path)
    module.__dict__.setdefault("__builtins__", builtins.__dict__)
    init = transform.build_module_init(source, module.__dict__)
    if callable(init):
        module._dp_module_init = init
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
