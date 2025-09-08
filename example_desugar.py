import dp_intrinsics
def _dp_gen_1(_dp_iter_1):
    _dp_iter_1 = getattr(dp_intrinsics, "iter")(_dp_iter_1)
    while True:
        try:
            i = getattr(dp_intrinsics, "next")(_dp_iter_1)
        except StopIteration:
            break
        if getattr(dp_intrinsics, "eq")(getattr(dp_intrinsics, "mod")(i, 2), 0):
            yield getattr(dp_intrinsics, "add")(i, 1)
def _dp_gen_2(_dp_iter_2):
    _dp_iter_2 = getattr(dp_intrinsics, "iter")(_dp_iter_2)
    while True:
        try:
            i = getattr(dp_intrinsics, "next")(_dp_iter_2)
        except StopIteration:
            break
        if getattr(dp_intrinsics, "eq")(getattr(dp_intrinsics, "mod")(i, 2), 0):
            yield getattr(dp_intrinsics, "add")(i, 1)
def _dp_gen_3(_dp_iter_3):
    _dp_iter_3 = getattr(dp_intrinsics, "iter")(_dp_iter_3)
    while True:
        try:
            i = getattr(dp_intrinsics, "next")(_dp_iter_3)
        except StopIteration:
            break
        if getattr(dp_intrinsics, "eq")(getattr(dp_intrinsics, "mod")(i, 2), 0):
            yield getattr(dp_intrinsics, "add")(i, 1)
sys = dp_intrinsics.import_("sys", __spec__)
ei = dp_intrinsics.import_("sys", __spec__, ["exc_info"]).exc_info
_dp_dec_1 = bar(1, 2)
def add(a, b):
    return getattr(dp_intrinsics, "add")(a, b)
add = foo(_dp_dec_1(add))
def _dp_ns_A(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_1 = __name__
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_1)
    getattr(dp_intrinsics, "setitem")(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_2 = "A"
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    getattr(dp_intrinsics, "setitem")(_ns, "__qualname__", _dp_tmp_2)
    _dp_tmp_3 = 1
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "b", _dp_tmp_3)
    getattr(dp_intrinsics, "setitem")(_ns, "b", _dp_tmp_3)

    def _mk___init__():

        def __init__(self):
            self.arr = list((1, 2, 3))
        __init__.__qualname__ = getattr(dp_intrinsics, "add")(getattr(dp_intrinsics, "getitem")(_ns, "__qualname__"), ".__init__")
        return __init__
    _dp_tmp_4 = _mk___init__()
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "__init__", _dp_tmp_4)
    getattr(dp_intrinsics, "setitem")(_ns, "__init__", _dp_tmp_4)

    def _mk_c():

        def c(self, d):
            return add(d, 2)
        c.__qualname__ = getattr(dp_intrinsics, "add")(getattr(dp_intrinsics, "getitem")(_ns, "__qualname__"), ".c")
        return c
    _dp_tmp_5 = _mk_c()
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "c", _dp_tmp_5)
    getattr(dp_intrinsics, "setitem")(_ns, "c", _dp_tmp_5)

    def _mk_test_aiter():

        async def test_aiter(self):
            _dp_iter_4 = getattr(dp_intrinsics, "iter")(range(10))
            while True:
                try:
                    i = getattr(dp_intrinsics, "next")(_dp_iter_4)
                except StopIteration:
                    break
                yield i
        test_aiter.__qualname__ = getattr(dp_intrinsics, "add")(getattr(dp_intrinsics, "getitem")(_ns, "__qualname__"), ".test_aiter")
        return test_aiter
    _dp_tmp_6 = _mk_test_aiter()
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "test_aiter", _dp_tmp_6)
    getattr(dp_intrinsics, "setitem")(_ns, "test_aiter", _dp_tmp_6)

    def _mk_d():

        async def d(self):
            _dp_iter_5 = getattr(dp_intrinsics, "aiter")(getattr(self, "test_aiter")())
            while True:
                try:
                    i = await getattr(dp_intrinsics, "anext")(_dp_iter_5)
                except StopAsyncIteration:
                    break
                print(i)
        d.__qualname__ = getattr(dp_intrinsics, "add")(getattr(dp_intrinsics, "getitem")(_ns, "__qualname__"), ".d")
        return d
    _dp_tmp_7 = _mk_d()
    getattr(dp_intrinsics, "setitem")(_dp_temp_ns, "d", _dp_tmp_7)
    getattr(dp_intrinsics, "setitem")(_ns, "d", _dp_tmp_7)
def _class_A():
    bases = getattr(dp_intrinsics, "resolve_bases")(())
    meta, ns, kwds = getattr(dp_intrinsics, "prepare_class")("A", bases)
    _dp_ns_A(ns)
    cls = meta("A", bases, ns)
    return cls
A = _class_A()
def ff():
    a = A()
    a.b = 5
    c = object()
    c.a = a
c = ff()
getattr(dp_intrinsics, "delattr")(getattr(c, "a"), "b")
getattr(dp_intrinsics, "delitem")(getattr(getattr(c, "a"), "arr"), 0)
del c
x = list(_dp_gen_1(getattr(dp_intrinsics, "iter")(range(5))))
y = set(_dp_gen_2(getattr(dp_intrinsics, "iter")(range(5))))
z = _dp_gen_3(getattr(dp_intrinsics, "iter")(range(5)))
