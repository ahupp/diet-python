__all__ = ["Example"]


class Example:
    pass


# diet-python: validate

def validate_module(module):
    actual = set(module.__all__)
    computed = {
        name
        for name, value in vars(module).items()
        if not name.startswith("__")
        and getattr(value, "__module__", None) == module.__name__
    }
    assert computed == actual
