def outer():
    class C:
        y = x

    x = 1
    return C

# diet-python: validate

def validate_module(module):

    import pytest

    with pytest.raises(NameError, match="cannot access free variable"):
        module.outer()
