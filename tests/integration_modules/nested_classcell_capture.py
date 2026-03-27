
def exercise():
    class C:
        def f(self):
            def g():
                return __class__
            return g()

    return C().f(), C


# diet-python: validate

def validate_module(module):
    value, cls = module.exercise()

    assert value is cls
