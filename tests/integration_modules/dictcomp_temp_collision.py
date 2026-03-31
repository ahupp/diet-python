def dict_comp_fib():
    a, b = 1, 2
    fib = {(c := a): (a := b) + (b := a + c) - b for __ in range(6)}
    return fib


# diet-python: validate

def validate_module(module):
    assert module.dict_comp_fib() == {
        1: 2,
        2: 3,
        3: 5,
        5: 8,
        8: 13,
        13: 21,
    }
