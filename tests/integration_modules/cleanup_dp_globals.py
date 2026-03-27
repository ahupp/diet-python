def has_dp_name() -> bool:
    return "_dp_name" in globals()

# diet-python: validate

def validate_module(module):

    assert module.has_dp_name() is False
