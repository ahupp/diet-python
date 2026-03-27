
class Example:
    a = __name__


# diet-python: validate

def validate_module(module):
    assert module.Example.a == module.__name__
