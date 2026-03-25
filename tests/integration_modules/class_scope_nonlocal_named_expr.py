def outer():
    x = 0

    class Box:
        nonlocal x
        y = (x := 1)

    return x, Box.y


result = outer()


# diet-python: validate


module = __import__("sys").modules[__name__]
assert module.result == (1, 1)
