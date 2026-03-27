
def genexpr_scope():
    a = 1
    gen = (b := a + i for i in range(2))
    return a, list(gen), b


# diet-python: validate

def validate_module(module):
    a, values, b = module.genexpr_scope()

    assert a == 1

    assert values == [1, 2]

    assert b == 2
