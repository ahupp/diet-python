def has_name(name: str) -> bool:
    return name in globals()

# diet-python: validate

assert has_name("_dp_name") is False
