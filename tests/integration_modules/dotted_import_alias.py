import dotted_import_alias_pkg.submodule as submodule

VALUE = submodule.SENTINEL
MODULE_NAME = submodule.__name__

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
assert module.VALUE == "submodule"
assert module.MODULE_NAME == "dotted_import_alias_pkg.submodule"
