

def run():
    l = [None]
    return [1 for (l[0], l) in [[1, 2]]]


# diet-python: validate

def validate_module(module):
    try:
        module.run()
    except UnboundLocalError:
        return

    raise AssertionError("Expected UnboundLocalError")
