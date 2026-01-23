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

def validate(module):
    assert module.RESULT is True
