import importlib.machinery
import importlib.util
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parent


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
        if spec and isinstance(spec.loader, importlib.machinery.SourceFileLoader):
            spec.loader = DietPythonLoader(fullname, spec.origin)
        return spec


def install():
    """Install the diet-python import hook."""
    if not any(isinstance(finder, DietPythonFinder) for finder in sys.meta_path):
        sys.meta_path.insert(0, DietPythonFinder)
