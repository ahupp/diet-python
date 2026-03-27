

def outer():
    x = 5
    def inner():
        nonlocal x
        x = 2
        return x
    return inner()


# diet-python: validate

def validate_module(module):
    assert module.outer() == 2
