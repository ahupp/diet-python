def get_yieldfrom_name():
    def a():
        yield

    def b():
        yield from a()
        yield

    gen_b = b()
    gen_b.send(None)
    return gen_b.gi_yieldfrom.gi_code.co_name
