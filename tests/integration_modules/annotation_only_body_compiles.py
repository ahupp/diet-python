
def f():
    a: int

RESULT = f()


# diet-python: validate

def validate_module(module):
    assert module.RESULT is None
