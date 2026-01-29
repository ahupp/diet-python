from __future__ import annotations

import functools


class Wrapper:
    def make_nested_class(self):
        class A:
            @functools.singledispatchmethod
            def func(self, arg: int) -> str:
                return str(arg)

        return A

    def bad_register_message(self):
        @functools.singledispatch
        def i(arg):
            return "base"

        try:
            @i.register
            def _(arg):
                return "missing annotation"
        except TypeError as exc:
            return str(exc)

        raise AssertionError("expected TypeError")

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
wrapper = module.Wrapper()
nested = wrapper.make_nested_class()
assert nested.func.__qualname__ == f"{nested.__qualname__}.func"

msg = module.Wrapper().bad_register_message()
assert "Invalid first argument to `register()`: " in msg
assert "Wrapper.bad_register_message.<locals>._" in msg
