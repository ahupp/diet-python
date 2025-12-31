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
