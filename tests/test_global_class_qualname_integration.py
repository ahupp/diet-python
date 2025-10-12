from __future__ import annotations

def test_global_class_has_module_qualname(run_integration_module):
    with run_integration_module("global_class_qualname") as module:
        qualname, inner_qualname = module.make_name()
        assert qualname == "Y"
        assert inner_qualname == "Y.Inner"
