def global_function():
    return (lambda: None).__qualname__, (lambda: None).__name__


RESULT = global_function()

# diet-python: validate

def validate_module(module):
    qualname, name = module.RESULT
    assert qualname == "global_function.<locals>.<lambda>"
    assert name == "<lambda>"
