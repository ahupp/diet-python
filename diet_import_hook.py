
import importlib.machinery
import linecache
import os
import sys
import tempfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent
_PYO3_TRANSFORM = None
INTEGRATION_ONLY = os.environ.get("DIET_PYTHON_INTEGRATION_ONLY") == "1"




def _transform_source(path: str) -> str:
    try:
        with open(path, "r", encoding="utf-8") as file:
            original_source = file.read()
    except OSError as err:
        raise ImportError(f"diet-python could not read source for {path}: {err}") from err
    transformer = _get_pyo3_transform()
    try:
        compiled_source = transformer.transform_source(original_source, True)
    except Exception as err:
        raise ImportError(f"diet-python failed for {path}: {err}") from err
    return compiled_source


def _get_pyo3_transform():
    global _PYO3_TRANSFORM
    if _PYO3_TRANSFORM is None:
        try:
            import diet_python as transform
        except Exception as err:
            transform = _load_pyo3_extension()
            if transform is None:
                raise ImportError("diet-python pyo3 module is required but could not be imported") from err
        _PYO3_TRANSFORM = transform
    return _PYO3_TRANSFORM


def _load_pyo3_extension():
    removed_indexes = []
    for index in range(len(sys.meta_path) - 1, -1, -1):
        if sys.meta_path[index] is DietPythonFinder:
            removed_indexes.append(index)
            sys.meta_path.pop(index)
    try:
        import importlib.machinery
        import importlib.util

        suffixes = set(importlib.machinery.EXTENSION_SUFFIXES)
        suffixes.update({".so", ".dylib", ".dll"})
        candidates = []
        for build in ("debug", "release"):
            base = REPO_ROOT / "target" / build
            for suffix in sorted(suffixes):
                candidates.append(base / f"libdiet_python{suffix}")
                candidates.append(base / f"diet_python{suffix}")
            if base.is_dir():
                for path in base.glob("libdiet_python*"):
                    candidates.append(path)
                for path in base.glob("diet_python*"):
                    candidates.append(path)
        for path in candidates:
            if not path.exists():
                continue
            try:
                spec = importlib.util.spec_from_file_location(
                    "diet_python",
                    path,
                    loader=importlib.machinery.ExtensionFileLoader("diet_python", str(path)),
                )
                if spec is None or spec.loader is None:
                    continue
                module = importlib.util.module_from_spec(spec)
                spec.loader.exec_module(module)
                sys.modules["diet_python"] = module
                return module
            except Exception:
                continue
        return None
    finally:
        for index in reversed(removed_indexes):
            sys.meta_path.insert(index, DietPythonFinder)



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
    try:
        resolved = Path(path).resolve()
    except OSError:
        return False
    if INTEGRATION_ONLY and not _is_integration_module(resolved):
        return False
    if os.environ.get("DIET_PYTHON_ALLOW_TEMP") != "1":
        try:
            resolved.relative_to(Path(tempfile.gettempdir()).resolve())
        except (OSError, ValueError):
            pass
        else:
            return False
    try:
        with open(path, "r", encoding="utf-8") as file:
            return "diet-python: disable" not in file.read()
    except OSError:
        return False


class DietPythonLoader(importlib.machinery.SourceFileLoader):
    """Loader that applies the diet-python transform before executing a module."""

    def get_code(self, fullname):
        source_bytes = self.get_data(self.path)
        return self.source_to_code(source_bytes, self.path)

    def source_to_code(self, data, path, *, _optimize=-1):
        source = _transform_source(path)
        return super().source_to_code(source.encode("utf-8"), path, _optimize=_optimize)


class DietPythonFinder(importlib.machinery.PathFinder):
    """Finder that wraps loaders to apply diet-python transformations."""

    @classmethod
    def find_spec(cls, fullname, path=None, target=None):
        spec = super().find_spec(fullname, path, target)
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

    if any(finder is DietPythonFinder for finder in sys.meta_path):
        return

    # Ensure the transform module is loaded before we intercept stdlib imports.
    _get_pyo3_transform()

    existing_typing = sys.modules.get("typing")
    if existing_typing is not None:
        loader = getattr(getattr(existing_typing, "__spec__", None), "loader", None)
        if not isinstance(loader, DietPythonLoader):
            sys.modules.pop("typing", None)

    for index, finder in enumerate(sys.meta_path):
        if finder is importlib.machinery.PathFinder:
            sys.meta_path.insert(index, DietPythonFinder)
            break
    else:
        sys.meta_path.insert(0, DietPythonFinder)
