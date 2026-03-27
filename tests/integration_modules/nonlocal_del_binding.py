

def outer():
    def gen():
        nonlocal value
        value = 10
        yield
    g = gen()
    next(g)
    assert value == 10
    del value
    return "ok"


def main():
    return outer()


# diet-python: validate

def validate_module(module):
    assert module.main() == "ok"
