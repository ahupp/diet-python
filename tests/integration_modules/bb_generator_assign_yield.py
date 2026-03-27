def gen():
    value = yield "start"
    yield value

# diet-python: validate

def validate_module(module):

    g = module.gen()
    assert next(g) == "start"
    assert g.send("x") == "x"
