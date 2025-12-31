import sys


def get_stack_names():
    def f():
        frame = sys._getframe()
        yield (frame.f_code.co_name, frame.f_back.f_code.co_name)

    def g():
        yield from f()

    gen = g()
    return gen.send(None)
