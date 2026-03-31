def run():
    ns = {}
    exec("x = 1", globals=ns)
    return ns["x"]


# diet-python: validate

def validate_module(module):
    assert module.run() == 1
