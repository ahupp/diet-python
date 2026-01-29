def format_debug():
    value = "A string"
    return f"{value=}"

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.format_debug() == "value='A string'"
