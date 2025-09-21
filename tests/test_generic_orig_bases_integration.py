from __future__ import annotations

import sys
from types import ModuleType

import diet_import_hook

from ._integration import transformed_module


def test_generic_orig_bases_preserved(run_integration_module):
    module_name = "generic_module"
    previous_typing = sys.modules.get("typing")
    sys.modules.pop("typing", None)

    try:
        with run_integration_module(module_name) as module:
            _assert_generic_module_invariants(module)
    finally:
        if previous_typing is not None:
            sys.modules["typing"] = previous_typing
        else:
            sys.modules.pop("typing", None)


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
