def run():
    ns = {}
    exec("global x\nx = 1", locals=ns)
    return ns


# diet-python: validate

def validate_module(module):
    assert module.run() == {}
