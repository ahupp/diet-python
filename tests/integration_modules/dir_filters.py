def run():
    junk = 1
    return dir()


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(NotImplementedError):
        module.run()
