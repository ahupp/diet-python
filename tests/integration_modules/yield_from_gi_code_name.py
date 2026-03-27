def get_name():
    def a():
        yield

    def b():
        yield from a()

    gen = b()
    gen.send(None)
    return gen.gi_yieldfrom.gi_code.co_name

# diet-python: validate

def validate_module(module):
    assert module.get_name() == "a"
