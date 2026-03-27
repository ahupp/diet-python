def outer():
    x = 0

    class Box:
        nonlocal x
        for x in [1]:
            pass

    return x


result = outer()

# diet-python: validate

def validate_module(module):
    assert module.result == 1
