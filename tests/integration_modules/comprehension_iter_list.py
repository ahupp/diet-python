
def run():
    magic_methods = "m"
    numerics = "n"
    inplace = "i"
    right = "r"
    return {
        "__%s__" % method for method in " ".join([magic_methods, numerics, inplace, right]).split()
    }


# diet-python: validate

def validate_module(module):
    assert module.run() == {"__m__", "__n__", "__i__", "__r__"}
