__file__ = "/tmp/site-packages/fake_module.py"

def f():
    return 1

# diet-python: validate

def validate_module(module):
    assert module.f() == 1
