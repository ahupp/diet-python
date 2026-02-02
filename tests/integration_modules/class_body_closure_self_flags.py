class CDLL:
    _func_flags_ = 1
    _func_restype_ = 2

    def __init__(self, use_errno=False, use_last_error=False):
        class _FuncPtr:
            _flags_ = self._func_flags_
            _restype_ = self._func_restype_
            if use_errno:
                _flags_ |= 4
            if use_last_error:
                _flags_ |= 8

        self._FuncPtr = _FuncPtr


def make():
    return CDLL(use_errno=True, use_last_error=True)


# diet-python: validate
obj = make()
assert obj._FuncPtr._flags_ == 1 | 4 | 8
assert obj._FuncPtr._restype_ == 2
