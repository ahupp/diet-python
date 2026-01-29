def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


RESULT = outer()

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.RESULT == "outer"
