def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


RESULT = outer()

# diet-python: validate

def validate_module(module):
    assert module.RESULT == "outer"
