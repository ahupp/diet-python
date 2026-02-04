
def run():
    log = []

    class C:
        def f(self):
            log.append("x")
            return log

    return C().f()


RESULT = run()

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.RESULT == ["x"]
