class Example:
    left, right = object(), object()


# diet-python: validate

def validate_module(module):
    assert hasattr(module.Example, "left")
    assert hasattr(module.Example, "right")
