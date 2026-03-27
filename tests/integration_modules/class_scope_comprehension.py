
class Example:
    values = [lc for lc in range(3)]


# diet-python: validate

def validate_module(module):
    assert module.Example.values == [0, 1, 2]
