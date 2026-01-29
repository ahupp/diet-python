def exercise():
    def recurse():
        return recurse()
    try:
        recurse()
    except RecursionError:
        return True
    return False


RESULT = exercise()

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.RESULT is True
