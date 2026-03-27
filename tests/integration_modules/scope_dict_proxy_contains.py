def has_name(name: str) -> bool:
    return name in globals()

# diet-python: validate

def validate_module(module):
    assert module.has_name("_dp_name") is False
