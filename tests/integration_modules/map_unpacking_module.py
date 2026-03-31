def summarize() -> tuple[int, int]:
    length_one, length_two = map(len, ("aa", "bbb"))
    return length_one, length_two


# diet-python: validate

def validate_module(module):
    assert module.summarize() == (2, 3)
