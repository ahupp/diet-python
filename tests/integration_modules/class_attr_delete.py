class Example:
    value = 1
    del value


EXPECTS_VALUE = hasattr(Example, "value")
