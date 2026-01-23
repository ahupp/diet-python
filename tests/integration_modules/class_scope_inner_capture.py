def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


RESULT = outer()

# diet-python: validate

def validate(module):
    assert module.RESULT == "outer"
