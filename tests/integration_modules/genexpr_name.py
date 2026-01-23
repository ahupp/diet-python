def get_genexpr_name():
    gen = (i for i in ())
    return gen.__name__

# diet-python: validate

def validate(module):
    assert module.get_genexpr_name() == "<genexpr>"
