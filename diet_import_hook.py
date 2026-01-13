import hashlib
import importlib.machinery
import importlib._bootstrap_external as _bootstrap_external
import io
import linecache
import os
import subprocess
import sys
import tempfile
import tokenize
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent
_BUILD_ATTEMPTED = False
_LINECACHE_PATCHED = False
_ORIGINAL_UPDATECACHE = None
_IO_OPEN_PATCHED = False
_ORIGINAL_IO_OPEN = None
_SOURCE_SHADOWS: dict[str, str] = {}
_SHADOW_ROOT = REPO_ROOT / ".diet_python_cache"


def _resolve_transform_cmd(path: str) -> list[str]:
    """Return the command to transform ``path``, preferring a built binary."""
    global _BUILD_ATTEMPTED
    bin_override = os.environ.get("DIET_PYTHON_BIN")
    if bin_override:
        bin_path = Path(bin_override)
    else:
        bin_path = REPO_ROOT / "target" / "debug" / "diet-python"

    if not _BUILD_ATTEMPTED and not bin_override:
        _BUILD_ATTEMPTED = True
        subprocess.run(
            ["cargo", "build", "--quiet", "--bin", "diet-python"],
            cwd=str(REPO_ROOT),
            check=True,
        )

    if not (bin_path.exists() and os.access(bin_path, os.X_OK)):
        subprocess.run(
            ["cargo", "build", "--quiet", "--bin", "diet-python"],
            cwd=str(REPO_ROOT),
            check=True,
        )

        if not (bin_path.exists() and os.access(bin_path, os.X_OK)):
            raise ImportError(f"failed to find diet-python after build, looking for {str(bin_path)}")

    return [str(bin_path), path]


def _transform_source(path: str) -> str:
    try:
        with open(path, "r", encoding="utf-8") as file:
            original_source = file.read()
    except OSError:
        original_source = ""
    cmd = _resolve_transform_cmd(path)
    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            cwd=str(REPO_ROOT),
            check=True,
        )
        compiled_source = proc.stdout
    except (OSError, subprocess.CalledProcessError) as err:
        print(f"diet-python failed for {path}: {err}", file=sys.stderr)
        try:
            with open(path, "r", encoding="utf-8") as file:
                compiled_source = file.read()
        except OSError as read_err:
            raise ImportError(f"diet-python could not read source for {path}: {read_err}") from err
    _SOURCE_SHADOWS[path] = compiled_source
    _update_linecache(path, compiled_source)
    return compiled_source


def _update_linecache(path: str, source: str) -> None:
    lines = source.splitlines(True)
    if lines and not lines[-1].endswith("\n"):
        lines[-1] += "\n"
    linecache.cache[path] = (len(source), None, lines, path)



def _should_transform(path: str) -> bool:
    """Return ``True`` if ``path`` should be passed through the transform."""
    try:
        resolved = Path(path).resolve()
    except OSError:
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
    global _LINECACHE_PATCHED
    global _ORIGINAL_UPDATECACHE
    global _IO_OPEN_PATCHED
    global _ORIGINAL_IO_OPEN

    if any(finder is DietPythonFinder for finder in sys.meta_path):
        return

    if not _LINECACHE_PATCHED:
        def _diet_updatecache(filename, module_globals=None):
            shadow = _SOURCE_SHADOWS.get(filename)
            if shadow is not None:
                _update_linecache(filename, shadow)
                return linecache.cache[filename][2]
            return _ORIGINAL_UPDATECACHE(filename, module_globals)

        _LINECACHE_PATCHED = True
        _ORIGINAL_UPDATECACHE = linecache.updatecache
        linecache.updatecache = _diet_updatecache

    for index, finder in enumerate(sys.meta_path):
        if finder is importlib.machinery.PathFinder:
            sys.meta_path.insert(index, DietPythonFinder)
            break
    else:
        sys.meta_path.insert(0, DietPythonFinder)
