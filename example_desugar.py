import __dp__
def _dp_gen_1(_dp_iter_1):
    _dp_iter_3 = getattr(__dp__, "iter")(_dp_iter_1)
    while True:
        try:
            i = getattr(__dp__, "next")(_dp_iter_3)
        except:
            _dp_exc_3 = getattr(__dp__, "current_exception")()
            if getattr(__dp__, "isinstance")(_dp_exc_3, StopIteration):
                break
            else:
                raise
        if getattr(__dp__, "eq")(getattr(__dp__, "mod")(i, 2), 0):
            yield getattr(__dp__, "add")(i, 1)
def _dp_gen_2(_dp_iter_2):
    _dp_iter_4 = getattr(__dp__, "iter")(_dp_iter_2)
    while True:
        try:
            i = getattr(__dp__, "next")(_dp_iter_4)
        except:
            _dp_exc_4 = getattr(__dp__, "current_exception")()
            if getattr(__dp__, "isinstance")(_dp_exc_4, StopIteration):
                break
            else:
                raise
        if getattr(__dp__, "eq")(getattr(__dp__, "mod")(i, 2), 0):
            yield getattr(__dp__, "add")(i, 1)
def _dp_gen_3(_dp_iter_3):
    _dp_iter_5 = getattr(__dp__, "iter")(_dp_iter_3)
    while True:
        try:
            i = getattr(__dp__, "next")(_dp_iter_5)
        except:
            _dp_exc_5 = getattr(__dp__, "current_exception")()
            if getattr(__dp__, "isinstance")(_dp_exc_5, StopIteration):
                break
            else:
                raise
        if getattr(__dp__, "eq")(getattr(__dp__, "mod")(i, 2), 0):
            yield getattr(__dp__, "add")(i, 1)
sys = getattr(__dp__, "import_")("sys", __spec__)
ei = getattr(getattr(__dp__, "import_")("sys", __spec__, list(("exc_info",))), "exc_info")
_dp_dec_1 = bar(1, 2)
def add(a, b):
    return getattr(__dp__, "add")(a, b)
add = foo(_dp_dec_1(add))
def _dp_ns_A(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    getattr(__dp__, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_1)
    getattr(__dp__, "setitem")(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "A"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    getattr(__dp__, "setitem")(_ns, "__qualname__", _dp_tmp_2)
    _dp_tmp_3 = 1
    getattr(__dp__, "setitem")(_dp_temp_ns, "b", _dp_tmp_3)
    getattr(__dp__, "setitem")(_ns, "b", _dp_tmp_3)

    def _mk___init__():

        def __init__(self):
            getattr(__dp__, "setattr")(self, "arr", list((1, 2, 3)))
        getattr(__dp__, "setattr")(__init__, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".__init__"))
        return __init__
    _dp_tmp_4 = _mk___init__()
    getattr(__dp__, "setitem")(_dp_temp_ns, "__init__", _dp_tmp_4)
    getattr(__dp__, "setitem")(_ns, "__init__", _dp_tmp_4)

    def _mk_c():

        def c(self, d):
            return add(d, 2)
        getattr(__dp__, "setattr")(c, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".c"))
        return c
    _dp_tmp_5 = _mk_c()
    getattr(__dp__, "setitem")(_dp_temp_ns, "c", _dp_tmp_5)
    getattr(__dp__, "setitem")(_ns, "c", _dp_tmp_5)

    def _mk_test_aiter():

        async def test_aiter(self):
            _dp_iter_1 = getattr(__dp__, "iter")(range(10))
            while True:
                try:
                    i = getattr(__dp__, "next")(_dp_iter_1)
                except:
                    _dp_exc_1 = getattr(__dp__, "current_exception")()
                    if getattr(__dp__, "isinstance")(_dp_exc_1, StopIteration):
                        break
                    else:
                        raise
                yield i
        getattr(__dp__, "setattr")(test_aiter, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".test_aiter"))
        return test_aiter
    _dp_tmp_6 = _mk_test_aiter()
    getattr(__dp__, "setitem")(_dp_temp_ns, "test_aiter", _dp_tmp_6)
    getattr(__dp__, "setitem")(_ns, "test_aiter", _dp_tmp_6)

    def _mk_d():

        async def d(self):
            _dp_iter_2 = getattr(__dp__, "aiter")(getattr(self, "test_aiter")())
            while True:
                try:
                    i = await getattr(__dp__, "anext")(_dp_iter_2)
                except:
                    _dp_exc_2 = getattr(__dp__, "current_exception")()
                    if getattr(__dp__, "isinstance")(_dp_exc_2, StopAsyncIteration):
                        break
                    else:
                        raise
                print(i)
        getattr(__dp__, "setattr")(d, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".d"))
        return d
    _dp_tmp_7 = _mk_d()
    getattr(__dp__, "setitem")(_dp_temp_ns, "d", _dp_tmp_7)
    getattr(__dp__, "setitem")(_ns, "d", _dp_tmp_7)
def _class_A():
    bases = getattr(__dp__, "resolve_bases")(())
    _dp_tmp_8 = getattr(__dp__, "prepare_class")("A", bases)
    meta = getattr(__dp__, "getitem")(_dp_tmp_8, 0)
    ns = getattr(__dp__, "getitem")(_dp_tmp_8, 1)
    kwds = getattr(__dp__, "getitem")(_dp_tmp_8, 2)
    _dp_ns_A(ns)
    cls = meta("A", bases, ns)
    return cls
A = _class_A()
def ff():
    a = A()
    getattr(__dp__, "setattr")(a, "b", 5)
    c = object()
    getattr(__dp__, "setattr")(c, "a", a)
c = ff()
getattr(__dp__, "delattr")(getattr(c, "a"), "b")
getattr(__dp__, "delitem")(getattr(getattr(c, "a"), "arr"), 0)
del c
x = list(_dp_gen_1(getattr(__dp__, "iter")(range(5))))
y = set(_dp_gen_2(getattr(__dp__, "iter")(range(5))))
z = _dp_gen_3(getattr(__dp__, "iter")(range(5)))

