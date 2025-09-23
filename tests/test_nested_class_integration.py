from __future__ import annotations


def test_nested_class_decorators_and_scope(run_integration_module):
    with run_integration_module("nested_classes") as module:
        outer = module.Outer

        assert module.calls == [
            ("inner_leaf", "Outer.Mid.Inner.Leaf", None),
            ("stack", "Outer.Mid.Inner", "inner"),
            ("mid_inner", "Outer.Mid.Inner", "inner"),
            ("outer_mid", "Outer.Mid", "mid"),
        ]

        assert outer.Mid.applied_decorators == ["outer_mid"]
        assert outer.Mid.Inner.applied_decorators == ["stack", "mid_inner"]
        assert outer.Mid.Inner.Leaf.applied_decorators == ["inner_leaf"]
        assert outer.Mid.Inner.Leaf.__qualname__ == "Outer.Mid.Inner.Leaf"
