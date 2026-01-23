import gc


class C:
    count = 0

    def __new__(cls):
        cls.count += 1
        return object.__new__(cls)

    def __del__(self):
        cls = self.__class__
        cls.count -= 1


def run():
    l = [C(), C(), C()]
    try:
        a, b = iter(l)
    except ValueError:
        pass
    del l
    gc.collect()
    return C.count


RESULT = run()

# diet-python: validate

def validate(module):
    assert module.RESULT == 0
