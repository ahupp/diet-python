def outer():
    x = 0

    class Box:
        nonlocal x
        x = 1

    return x


result = outer()

# diet-python: validate

def validate_module(module):
    assert module.result == 1
