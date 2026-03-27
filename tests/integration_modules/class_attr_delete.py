class Example:
    value = 1
    del value


EXPECTS_VALUE = hasattr(Example, "value")

# diet-python: validate

def validate_module(module):

    assert module.EXPECTS_VALUE is False
