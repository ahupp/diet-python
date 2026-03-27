
def make():
    def inner(a=1, *, b=2):
        return a, b
    return inner

def run():
    inner = make()
    inner.__defaults__ = (10,)
    inner.__kwdefaults__ = {"b": 20}
    return inner()


# diet-python: validate

def validate_module(module):
    assert module.run() == (10, 20)
