def outer():
    x = 1

    def inner():
        return x

    return inner

# diet-python: validate

def validate_module(module):
    inner = module.outer()
    assert inner.__closure__ is not None
