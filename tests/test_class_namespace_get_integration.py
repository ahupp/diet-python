from __future__ import annotations


def test_class_namespace_get_method_does_not_break_classmethod_lookup(
    run_integration_module,
):
    with run_integration_module("class_namespace_get") as module:
        assert module.RESULT == module.Example[int]
