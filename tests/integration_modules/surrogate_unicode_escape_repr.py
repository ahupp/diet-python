def repr_value():
    char = "\uDCBA"
    return repr(char)


def ascii_value():
    char = "\uDCBA"
    return ascii(char)

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.repr_value() == "'\\udcba'"
assert module.ascii_value() == "'\\udcba'"
