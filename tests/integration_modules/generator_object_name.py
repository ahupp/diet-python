def make_gen():
    yield 1


# diet-python: validate

module = __import__("sys").modules[__name__]
gen = module.make_gen()
assert gen.__name__ == "make_gen"
