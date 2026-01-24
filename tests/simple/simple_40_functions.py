def add(a, b):
    return a + b


def double(n):
    return add(n, n)


RESULT = double(4)

# diet-python: validate

def validate(module):
    assert module.add(2, 3) == 5
    assert module.RESULT == 8
