class CDLL:
    _func_flags_ = 7

    def __init__(self):
        class _FuncPtr:
            x = self._func_flags_

        self._FuncPtr = _FuncPtr


def make():
    return CDLL()


# diet-python: validate
obj = make()
assert obj._FuncPtr.x == 7
