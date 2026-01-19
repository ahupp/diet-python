def global_function():
    return (lambda: None).__qualname__, (lambda: None).__name__


RESULT = global_function()
