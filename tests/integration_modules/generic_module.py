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

import sys
from types import ModuleType

import diet_import_hook




def _assert_generic_module_invariants(module: ModuleType) -> None:
    transformed_typing = sys.modules["typing"]
    assert isinstance(
        transformed_typing.__spec__.loader, diet_import_hook.DietPythonLoader
    ), "typing should be transformed"

    assert "__dp__" in module.__dict__, "module should be transformed"

    assert module.Box.__orig_bases__ == (transformed_typing.Generic[module.T],)

    specialized = module.make_specialization()
    assert specialized.__orig_bases__[0].__args__ == (int,)
    assert issubclass(specialized, module.Box)

def validate(module):
    _assert_generic_module_invariants(module)
