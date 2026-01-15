from __future__ import annotations


def test_listcomp_classcell(run_integration_module):
    with run_integration_module("listcomp_classcell") as module:
        values, method_class, cls = module.classcell_values()
        assert values == [4, 4, 4, 4, 4]
        assert method_class is cls
