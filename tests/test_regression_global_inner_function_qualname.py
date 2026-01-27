from tests._integration import transformed_module


def test_global_inner_function_qualname(tmp_path):
    source = """
def build_qualnames():
    def global_function():
        def inner_function():
            global inner_global_function

            def inner_global_function():
                def inner_function2():
                    pass

                return inner_function2

            return inner_global_function

        return inner_function()

    inner_fn = global_function()
    return inner_global_function.__qualname__, inner_fn().__qualname__

RESULT = build_qualnames()
"""
    with transformed_module(tmp_path, "global_inner_function_qualname_regression", source) as module:
        qualname, inner_qualname = module.RESULT
        assert qualname == "inner_global_function"
        assert inner_qualname == "inner_global_function.<locals>.inner_function2"
