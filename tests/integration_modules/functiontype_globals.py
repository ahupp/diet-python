import types


def make_inner():
    def inner():
        return 1 + 1

    return inner


def run():
    inner_code = make_inner().__code__
    func = types.FunctionType(inner_code, {})
    return func()


# diet-python: validate

def validate_module(module):
    assert module.run() == 2
