def make_gen():
    yield 1

# diet-python: validate

def validate_module(module):
    gen = module.make_gen()
    assert gen.__name__ == "make_gen"
