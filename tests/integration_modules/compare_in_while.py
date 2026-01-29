
def loop_compare(a, b):
    while True:
        if a == b:
            return True
        return False

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.loop_compare(1, 1) is True
assert module.loop_compare(1, 2) is False
