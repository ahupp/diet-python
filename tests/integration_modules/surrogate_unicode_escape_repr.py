def repr_value():
    char = "\uDCBA"
    return repr(char)


def ascii_value():
    char = "\uDCBA"
    return ascii(char)

# diet-python: validate

def validate(module):
    assert module.repr_value() == "'\\udcba'"
    assert module.ascii_value() == "'\\udcba'"
