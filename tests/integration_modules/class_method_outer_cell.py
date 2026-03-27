
def run():
    log = []

    class C:
        def f(self):
            log.append("x")
            return log

    return C().f()


RESULT = run()

# diet-python: validate

def validate_module(module):
    assert module.RESULT == ["x"]
