calls = []


def record(tag: str):
    def decorator(cls):
        calls.append((tag, cls.__qualname__, getattr(cls, "label", None)))
        applied = list(getattr(cls, "applied_decorators", []))
        applied.append(tag)
        cls.applied_decorators = applied
        return cls

    return decorator


class Outer:
    label = "outer"

    @record(label + "_mid")
    class Mid:
        label = "mid"

        @record(label + "_inner")
        @record("stack")
        class Inner:
            label = "inner"

            @record(label + "_leaf")
            class Leaf:
                pass

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
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
