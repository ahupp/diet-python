class Example:
    a = b = object()


# diet-python: validate

def validate_module(module):
    assert module.Example.a is module.Example.b
