from __future__ import annotations

import sys

import diet_import_hook

from ._integration import transformed_module

MODULE_SOURCE = """
from typing import Generic, TypeVar


T = TypeVar("T")


class Box(Generic[T]):
    pass


def make_specialization():
    class IntBox(Box[int]):
        pass
    return IntBox
"""


def test_generic_orig_bases_preserved(tmp_path):
    previous_typing = sys.modules.get("typing")
    sys.modules.pop("typing", None)

    try:
        with transformed_module(tmp_path, "generic_module", MODULE_SOURCE) as module:
            transformed_typing = sys.modules["typing"]
            assert isinstance(
                transformed_typing.__spec__.loader, diet_import_hook.DietPythonLoader
            ), "typing should be transformed"

            assert "__dp__" in module.__dict__, "module should be transformed"

            assert module.Box.__orig_bases__ == (transformed_typing.Generic[module.T],)

            specialized = module.make_specialization()
            assert specialized.__orig_bases__[0].__args__ == (int,)
            assert issubclass(specialized, module.Box)
    finally:
        sys.modules.pop("generic_module", None)
        if previous_typing is not None:
            sys.modules["typing"] = previous_typing
        else:
            sys.modules.pop("typing", None)
