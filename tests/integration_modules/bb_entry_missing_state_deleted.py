
def f():
    try:
        pass
    except Exception:
        pass
    return 1

y = f()


# diet-python: validate

def validate_module(module):
    assert module.y == 1
