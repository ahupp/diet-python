import ctypes

# diet-python: validate

def validate_module(module):
    assert module.ctypes is not None
