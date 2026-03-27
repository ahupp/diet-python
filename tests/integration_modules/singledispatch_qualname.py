
import functools

class Wrapper:
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

def validate_module(module):
    msg = module.Wrapper().bad_register_message()

    assert "Wrapper.bad_register_message.<locals>._" in msg
