class Box:
    x = 1
    del x


result = hasattr(Box, "x")

# diet-python: validate

def validate_module(module):
    assert module.result is False
