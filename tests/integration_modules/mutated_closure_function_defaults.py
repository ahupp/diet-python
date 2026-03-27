
def make():
    sentinel = object()
    def inner(value=sentinel):
        return value
    return inner

def run():
    inner = make()
    replacement = object()
    inner.__defaults__ = (replacement,)
    return inner() is replacement


# diet-python: validate

def validate_module(module):
    assert module.run() is True
