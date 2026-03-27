def make_name():
    global Y

    class Y:
        class Inner:
            pass

    return Y.__qualname__, Y.Inner.__qualname__

# diet-python: validate

def validate_module(module):

    qualname, inner_qualname = module.make_name()
    assert qualname == "Y"
    assert inner_qualname == "Y.Inner"
