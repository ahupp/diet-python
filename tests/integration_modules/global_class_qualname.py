def make_name():
    global Y

    class Y:
        class Inner:
            pass

    return Y.__qualname__, Y.Inner.__qualname__
