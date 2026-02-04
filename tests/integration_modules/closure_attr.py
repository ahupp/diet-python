def outer():
    x = 1

    def inner():
        return x

    return inner

# diet-python: validate

module = __import__("sys").modules[__name__]
inner = module.outer()
assert inner.__closure__ is not None
