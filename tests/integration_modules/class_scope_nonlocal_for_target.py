def outer():
    x = 0

    class Box:
        nonlocal x
        for x in [1]:
            pass

    return x


result = outer()


# diet-python: validate


module = __import__("sys").modules[__name__]
assert module.result == 1
