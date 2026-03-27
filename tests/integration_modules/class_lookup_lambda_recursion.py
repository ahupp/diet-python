class Example:
    value = __name__


RESULT = Example.value

# diet-python: validate

def validate_module(module):

    assert module.RESULT == module.__name__
