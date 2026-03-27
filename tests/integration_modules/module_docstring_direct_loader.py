"""module docs"""

VALUE = 1


# diet-python: validate

def validate_module(module):
    assert module.__doc__ == "module docs"

    assert module.VALUE == 1
