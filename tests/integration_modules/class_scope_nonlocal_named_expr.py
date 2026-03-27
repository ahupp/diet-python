def outer():
    x = 0

    class Box:
        nonlocal x
        y = (x := 1)

    return x, Box.y


result = outer()

# diet-python: validate

def validate_module(module):
    assert module.result == (1, 1)
