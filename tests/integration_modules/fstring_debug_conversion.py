def format_debug():
    value = "A string"
    return f"{value=}"

# diet-python: validate

def validate_module(module):
    assert module.format_debug() == "value='A string'"
