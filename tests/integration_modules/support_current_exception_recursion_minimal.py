def exercise():
    def recurse():
        return recurse()
    try:
        recurse()
    except RecursionError:
        return True
    return False


RESULT = exercise()
