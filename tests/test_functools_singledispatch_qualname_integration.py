from __future__ import annotations


def test_singledispatchmethod_qualname(run_integration_module):
    with run_integration_module("functools_singledispatch_qualname") as module:
        wrapper = module.Wrapper()
        nested = wrapper.make_nested_class()
        assert nested.func.__qualname__ == f"{nested.__qualname__}.func"


def test_singledispatch_register_error_message(run_integration_module):
    with run_integration_module("functools_singledispatch_qualname") as module:
        msg = module.Wrapper().bad_register_message()
        assert "Invalid first argument to `register()`: " in msg
        assert "Wrapper.bad_register_message.<locals>._" in msg
