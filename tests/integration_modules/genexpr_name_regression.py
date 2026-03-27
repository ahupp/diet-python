
def get_name():
    gen = (i for i in ())
    return gen.__name__


# diet-python: validate

def validate_module(module):
    assert module.get_name() == "<genexpr>"
