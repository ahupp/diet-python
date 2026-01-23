def format_debug():
    value = "A string"
    return f"{value=}"

# diet-python: validate

def validate(module):
    assert module.format_debug() == "value='A string'"
