def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


RESULT = outer()
