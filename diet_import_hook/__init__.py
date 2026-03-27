
import importlib.machinery
import builtins
import linecache
import os
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent.parent
_PYO3_TRANSFORM = None
def _integration_only_enabled() -> bool:
    # Read dynamically so tests can toggle this per import context.
    return os.environ.get("DIET_PYTHON_INTEGRATION_ONLY") == "1"


def _transform_source(path: str, module_name: str | None = None) -> str:
    try:
        with open(path, "r", encoding="utf-8") as file:
            original_source = file.read()
    except OSError as err:
        raise ImportError(f"diet-python could not read source for {path}: {err}") from err
    transformer = _get_pyo3_transform()
    try:
        if module_name and hasattr(transformer, "transform_source_with_name"):
            compiled_source = transformer.transform_source_with_name(
                original_source, module_name, True
            )
        else:
            compiled_source = transformer.transform_source(original_source, True)
    except SyntaxError as err:
        if err.filename is None:
            err.filename = path
        raise
    except Exception as err:
        raise ImportError(f"diet-python failed for {path}: {err}") from err
    return compiled_source


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
        init, doc = transformer.build_module_init(
            original_source,
            module.__dict__,
            True,
        )
    except SyntaxError as err:
        if err.filename is None:
            err.filename = path
        raise
    except Exception as err:
        raise ImportError(f"diet-python failed for {path}: {err}") from err
    return init, doc


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
        init, doc = _build_module_init(self.path, module)
        if doc is not None:
            module.__doc__ = doc
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
        import __dp__ as _dp_module
        hook_fn = getattr(_dp_module, "_ensure_annotationlib_import_hook", None)
        if hook_fn is not None:
            hook_fn()
        transform = _get_pyo3_transform()
        _dp_module._jit_make_bb_function = getattr(transform, "make_bb_function", None)
        _dp_module._jit_make_bb_generator = getattr(transform, "make_bb_generator", None)
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
