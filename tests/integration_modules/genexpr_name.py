def get_genexpr_name():
    gen = (i for i in ())
    return gen.__name__

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.get_genexpr_name() == "<genexpr>"
