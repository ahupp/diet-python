import __dp__
sys = getattr(__dp__, "import_")("sys", __spec__)
ei = getattr(getattr(__dp__, "import_")("sys", __spec__, list(("exc_info",))), "exc_info")
def _dp_dec_apply_1(_dp_the_func):
    return foo(bar(1, 2)(_dp_the_func))
def add(a, b):
    return getattr(__dp__, "add")(a, b)
add = _dp_dec_apply_1(add)
def _dp_ns_A(_ns):
    _dp_temp_ns = dict(())
    _dp_tmp_2 = __name__
    getattr(__dp__, "setitem")(_dp_temp_ns, "__module__", _dp_tmp_2)
    getattr(__dp__, "setitem")(_ns, "__module__", _dp_tmp_2)
    _dp_tmp_3 = "A"
    getattr(__dp__, "setitem")(_dp_temp_ns, "__qualname__", _dp_tmp_3)
    getattr(__dp__, "setitem")(_ns, "__qualname__", _dp_tmp_3)
    _dp_tmp_4 = 1
    getattr(__dp__, "setitem")(_dp_temp_ns, "b", _dp_tmp_4)
    getattr(__dp__, "setitem")(_ns, "b", _dp_tmp_4)

    def _dp_mk___init__():

        def __init__(self):
            getattr(__dp__, "setattr")(self, "arr", list((1, 2, 3)))
        getattr(__dp__, "setattr")(__init__, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".__init__"))
        return __init__
    _dp_tmp_5 = _dp_mk___init__()
    getattr(__dp__, "setitem")(_dp_temp_ns, "__init__", _dp_tmp_5)
    getattr(__dp__, "setitem")(_ns, "__init__", _dp_tmp_5)

    def _dp_mk_c():

        def c(self, d):
            return add(d, 2)
        getattr(__dp__, "setattr")(c, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".c"))
        return c
    _dp_tmp_6 = _dp_mk_c()
    getattr(__dp__, "setitem")(_dp_temp_ns, "c", _dp_tmp_6)
    getattr(__dp__, "setitem")(_ns, "c", _dp_tmp_6)

    def _dp_mk_test_aiter():

        async def test_aiter(self):
            _dp_iter_7 = getattr(__dp__, "iter")(range(10))
            while True:
                try:
                    i = getattr(__dp__, "next")(_dp_iter_7)
                except:
                    getattr(__dp__, "check_stopiteration")()
                    break
                else:
                    yield i
        getattr(__dp__, "setattr")(test_aiter, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".test_aiter"))
        return test_aiter
    _dp_tmp_8 = _dp_mk_test_aiter()
    getattr(__dp__, "setitem")(_dp_temp_ns, "test_aiter", _dp_tmp_8)
    getattr(__dp__, "setitem")(_ns, "test_aiter", _dp_tmp_8)

    def _dp_mk_d():

        async def d(self):
            _dp_iter_9 = getattr(__dp__, "aiter")(getattr(self, "test_aiter")())
            while True:
                try:
                    i = await getattr(__dp__, "anext")(_dp_iter_9)
                except:
                    getattr(__dp__, "acheck_stopiteration")()
                    break
                else:
                    print(i)
        getattr(__dp__, "setattr")(d, "__qualname__", getattr(__dp__, "add")(getattr(__dp__, "getitem")(_ns, "__qualname__"), ".d"))
        return d
    _dp_tmp_10 = _dp_mk_d()
    getattr(__dp__, "setitem")(_dp_temp_ns, "d", _dp_tmp_10)
    getattr(__dp__, "setitem")(_ns, "d", _dp_tmp_10)
def _dp_make_class_A():
    bases = getattr(__dp__, "resolve_bases")(())
    _dp_tmp_11 = getattr(__dp__, "prepare_class")("A", bases, None)
    meta = getattr(__dp__, "getitem")(_dp_tmp_11, 0)
    ns = getattr(__dp__, "getitem")(_dp_tmp_11, 1)
    kwds = getattr(__dp__, "getitem")(_dp_tmp_11, 2)
    _dp_ns_A(ns)
    return meta("A", bases, ns, **kwds)
_dp_tmp_12 = _dp_make_class_A()
A = _dp_tmp_12
_dp_class_A = _dp_tmp_12
def ff():
    a = A()
    getattr(__dp__, "setattr")(a, "b", 5)
    c = object()
    getattr(__dp__, "setattr")(c, "a", a)
c = ff()
getattr(__dp__, "delattr")(getattr(c, "a"), "b")
getattr(__dp__, "delitem")(getattr(getattr(c, "a"), "arr"), 0)
del c
_dp_tmp_13 = range(5)
_dp_tmp_14 = getattr(__dp__, "mod")(i, 2)
_dp_tmp_15 = getattr(__dp__, "eq")(_dp_tmp_14, 0)
_dp_tmp_16 = getattr(__dp__, "add")(i, 1)
def _dp_gen_18(_dp_tmp_13):
    _dp_iter_19 = getattr(__dp__, "iter")(_dp_tmp_13)
    while True:
        try:
            i = getattr(__dp__, "next")(_dp_iter_19)
        except:
            getattr(__dp__, "check_stopiteration")()
            break
        else:
            if _dp_tmp_15:
                yield _dp_tmp_16
_dp_tmp_17 = list(_dp_gen_18(getattr(__dp__, "iter")(_dp_tmp_13)))
x = _dp_tmp_17
_dp_tmp_20 = range(5)
_dp_tmp_21 = getattr(__dp__, "mod")(i, 2)
_dp_tmp_22 = getattr(__dp__, "eq")(_dp_tmp_21, 0)
_dp_tmp_23 = getattr(__dp__, "add")(i, 1)
def _dp_gen_25(_dp_tmp_20):
    _dp_iter_26 = getattr(__dp__, "iter")(_dp_tmp_20)
    while True:
        try:
            i = getattr(__dp__, "next")(_dp_iter_26)
        except:
            getattr(__dp__, "check_stopiteration")()
            break
        else:
            if _dp_tmp_22:
                yield _dp_tmp_23
_dp_tmp_24 = set(_dp_gen_25(getattr(__dp__, "iter")(_dp_tmp_20)))
y = _dp_tmp_24
_dp_tmp_27 = range(5)
_dp_tmp_28 = getattr(__dp__, "mod")(i, 2)
_dp_tmp_29 = getattr(__dp__, "eq")(_dp_tmp_28, 0)
_dp_tmp_30 = getattr(__dp__, "add")(i, 1)
def _dp_gen_32(_dp_tmp_27):
    _dp_iter_33 = getattr(__dp__, "iter")(_dp_tmp_27)
    while True:
        try:
            i = getattr(__dp__, "next")(_dp_iter_33)
        except:
            getattr(__dp__, "check_stopiteration")()
            break
        else:
            if _dp_tmp_29:
                yield _dp_tmp_30
_dp_tmp_31 = _dp_gen_32(getattr(__dp__, "iter")(_dp_tmp_27))
z = _dp_tmp_31
