from tests._integration import transformed_module


def test_singledispatch_register_qualname(tmp_path):
    source = """
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
"""
    with transformed_module(tmp_path, "singledispatch_qualname", source) as module:
        msg = module.Wrapper().bad_register_message()
        assert "Wrapper.bad_register_message.<locals>._" in msg
