from __future__ import annotations


def test_method_docstring_preserved(run_integration_module):
    with run_integration_module("method_docstring") as module:
        assert module.Example.do_thing.__doc__ == "Example command."
        assert module.Example.do_thing.__annotations__ == {"value": int, "return": int}
        assert module.build_annotations(module.Example) is int

