import importlib.machinery
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent


def _should_transform(path: str) -> bool:
    """Return ``True`` if ``path`` should be passed through the transform."""

    path_obj = Path(path).resolve()
    if not path_obj.is_relative_to(REPO_ROOT / "tests"):
        return False
    try:
        with open(path, "r", encoding="utf-8") as file:
            return "diet-python: disable" not in file.read()
    except OSError:
        return False


class DietPythonLoader(importlib.machinery.SourceFileLoader):
    """Loader that applies the diet-python transform before executing a module."""

    def source_to_code(self, data, path, *, _optimize=-1):
        cmd = ["cargo", "run", "--quiet", "--", path]
        try:
            proc = subprocess.run(
                cmd,
                capture_output=True,
                text=True,
                cwd=str(REPO_ROOT),
                check=True,
            )
        except (OSError, subprocess.CalledProcessError) as err:
            raise ImportError(f"diet-python failed for {path}: {err}") from err
        transformed = proc.stdout
        return super().source_to_code(transformed.encode("utf-8"), path, _optimize=_optimize)


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

    for index, finder in enumerate(sys.meta_path):
        if finder is importlib.machinery.PathFinder:
            sys.meta_path[index] = DietPythonFinder
            break
    else:
        sys.meta_path.insert(0, DietPythonFinder)
