
value = 0

def run():
    exec("value = 1", globals())
    return value


# diet-python: validate

def validate_module(module):
    assert module.run() == 1
