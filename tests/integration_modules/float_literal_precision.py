VALUE = 0.9999999999999999
RESULT = VALUE < 1.0

# diet-python: validate

def validate_module(module):

    assert module.RESULT is True
