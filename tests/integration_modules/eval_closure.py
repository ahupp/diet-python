def run():
    d1 = 10
    d2 = 32

    def inner():
        _ = (d1, d2)
        return eval("d1 + d2")

    return inner()


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(NotImplementedError):
        module.run()
