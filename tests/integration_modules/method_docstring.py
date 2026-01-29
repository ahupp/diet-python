class Example:
    def do_thing(self, value: int) -> int:
        """Example command."""
        return value


def build_help(cls):
    for name in ("thing",):
        method = getattr(cls, f"do_{name}")
        method.__doc__.strip()
        method.__annotations__["value"]


def build_annotations(cls):
    return cls.do_thing.__annotations__["return"]


build_help(Example)

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.Example.do_thing.__doc__ == "Example command."
assert module.Example.do_thing.__annotations__ == {"value": int, "return": int}
assert module.build_annotations(module.Example) is int
