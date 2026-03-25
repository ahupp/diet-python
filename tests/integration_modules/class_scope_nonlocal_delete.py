def outer():
    x = "outer"

    class Box:
        nonlocal x
        del x

    try:
        x
    except UnboundLocalError:
        return "deleted"
    return "still-bound"


RESULT = outer()
assert RESULT == "deleted"
