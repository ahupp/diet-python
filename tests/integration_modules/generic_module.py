from typing import Generic, TypeVar


T = TypeVar("T")


class Box(Generic[T]):
    pass


def make_specialization():
    class IntBox(Box[int]):
        pass

    return IntBox

# diet-python: validate

from __future__ import annotations

import os
import sys
import builtins
from types import ModuleType

import diet_import_hook




def _assert_generic_module_invariants(module: ModuleType) -> None:
    transformed_typing = sys.modules["typing"]
    if __dp_integration_transformed__:
        if os.environ.get("DIET_PYTHON_INTEGRATION_ONLY") != "1":
            assert isinstance(
                transformed_typing.__spec__.loader, diet_import_hook.DietPythonLoader
            ), "typing should be transformed"
        assert "__dp__" not in module.__dict__, "__dp__ should not be injected into module globals"
        assert hasattr(builtins, "__dp__"), "__dp__ runtime should be available via builtins"

    assert module.Box.__orig_bases__ == (transformed_typing.Generic[module.T],)

    specialized = module.make_specialization()
    assert specialized.__orig_bases__[0].__args__ == (int,)
    assert issubclass(specialized, module.Box)

module = __import__("sys").modules[__name__]
_assert_generic_module_invariants(module)
