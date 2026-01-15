import importlib.abc
import importlib.machinery
import sys

from soac_exec import CraneLoaderExt


class CraneFinder(importlib.abc.MetaPathFinder):
    def __init__(self):
        self.loader = CraneLoaderExt(self)

    def find_spec(self, fullname, path, target=None):
        spec = importlib.machinery.PathFinder.find_spec(fullname, path, target)
        if not spec or not spec.origin or not spec.origin.endswith(".py"):
            return None
        if not self.loader.is_strict_module(spec.origin):
            return spec
        spec.loader = self.loader
        return spec


def install():
    if not any(isinstance(f, CraneFinder) for f in sys.meta_path):
        sys.meta_path.insert(0, CraneFinder())
