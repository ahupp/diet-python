def get_name():
    def a():
        yield

    def b():
        yield from a()

    gen = b()
    gen.send(None)
    return gen.gi_yieldfrom.gi_code.co_name


# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.get_name() == "a"
