
def outer():
    x = 1
    def inner():
        nonlocal x
        del x
        return "ok"
    inner()
    return "done"

RESULT = outer()


# diet-python: validate

def validate_module(module):
    assert module.RESULT == "done"
