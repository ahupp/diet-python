def run():
    x = 10
    code = compile("x + 1", "", "exec")
    exec(code)
    return True


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(NotImplementedError):
        module.run()
