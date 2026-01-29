class My:
    def __repr__[T: str](self, default: T = "") -> str:
        return default


def run():
    return My().__repr__()


RESULT = run()

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.RESULT == ""
