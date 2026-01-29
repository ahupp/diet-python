def global_function():
    return (lambda: None).__qualname__, (lambda: None).__name__

# diet-python: validate

module = __import__("sys").modules[__name__]
qualname, name = module.global_function()
assert qualname == "global_function.<locals>.<lambda>"
assert name == "<lambda>"
