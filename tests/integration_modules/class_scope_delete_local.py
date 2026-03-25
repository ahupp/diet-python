class Box:
    x = 1
    del x


result = hasattr(Box, "x")


# diet-python: validate


module = __import__("sys").modules[__name__]
assert module.result is False
