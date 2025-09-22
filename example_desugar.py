import __dp__
sys = __dp__.import_("sys", __spec__)
ei = __dp__.import_("sys", __spec__, __dp__.list(("exc_info",))).exc_info
_dp_decorator_add_0 = foo
_dp_decorator_add_1 = bar(1, 2)
def add(a, b):
    return __dp__.add(a, b)
add = _dp_decorator_add_0(_dp_decorator_add_1(add))
def _dp_ns_A(_dp_prepare_ns):
    _dp_temp_ns = __dp__.dict()
    __dp__.setitem(_dp_temp_ns, "__module__", __name__)
    __dp__.setitem(_dp_prepare_ns, "__module__", __name__)
    _dp_tmp_1 = "A"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_1)
    __dp__.setitem(_dp_prepare_ns, "__qualname__", _dp_tmp_1)
    _dp_class_annotations = _dp_temp_ns.get("__annotations__")
    _dp_tmp_2 = __dp__.is_(_dp_class_annotations, None)
    if _dp_tmp_2:
        _dp_class_annotations = __dp__.dict()
    b = 1
    __dp__.setitem(_dp_temp_ns, "b", b)
    __dp__.setitem(_dp_prepare_ns, "b", b)

    def _dp_mk___init__():

        def __init__(self):
            __dp__.setattr(self, "arr", __dp__.list((1, 2, 3)))
        __dp__.setattr(__init__, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".__init__"))
        return __init__
    __init__ = _dp_mk___init__()
    __dp__.setitem(_dp_temp_ns, "__init__", __init__)
    __dp__.setitem(_dp_prepare_ns, "__init__", __init__)

    def _dp_mk_c():

        def c(self, d):
            return add(d, 2)
        __dp__.setattr(c, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".c"))
        return c
    c = _dp_mk_c()
    __dp__.setitem(_dp_temp_ns, "c", c)
    __dp__.setitem(_dp_prepare_ns, "c", c)

    def _dp_mk_test_aiter():

        async def test_aiter(self):
            _dp_iter_3 = __dp__.iter(range(10))
            while True:
                try:
                    i = __dp__.next(_dp_iter_3)
                except:
                    __dp__.check_stopiteration()
                    break
                else:
                    yield i
        __dp__.setattr(test_aiter, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".test_aiter"))
        return test_aiter
    test_aiter = _dp_mk_test_aiter()
    __dp__.setitem(_dp_temp_ns, "test_aiter", test_aiter)
    __dp__.setitem(_dp_prepare_ns, "test_aiter", test_aiter)

    def _dp_mk_d():

        async def d(self):
            _dp_iter_4 = __dp__.aiter(self.test_aiter())
            while True:
                try:
                    i = await __dp__.anext(_dp_iter_4)
                except:
                    __dp__.acheck_stopiteration()
                    break
                else:
                    print(i)
        __dp__.setattr(d, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".d"))
        return d
    d = _dp_mk_d()
    __dp__.setitem(_dp_temp_ns, "d", d)
    __dp__.setitem(_dp_prepare_ns, "d", d)
def _dp_make_class_A():
    orig_bases = ()
    bases = __dp__.resolve_bases(orig_bases)
    _dp_tmp_5 = __dp__.prepare_class("A", bases, None)
    meta = __dp__.getitem(_dp_tmp_5, 0)
    ns = __dp__.getitem(_dp_tmp_5, 1)
    kwds = __dp__.getitem(_dp_tmp_5, 2)
    _dp_ns_A(ns)
    _dp_tmp_7 = __dp__.is_not(orig_bases, bases)
    _dp_tmp_6 = _dp_tmp_7
    if _dp_tmp_6:
        _dp_tmp_8 = __dp__.not_(__dp__.contains(ns, "__orig_bases__"))
        _dp_tmp_6 = _dp_tmp_8
    if _dp_tmp_6:
        __dp__.setitem(ns, "__orig_bases__", orig_bases)
    return meta("A", bases, ns, **kwds)
_dp_class_A = _dp_make_class_A()
A = _dp_class_A
def ff():
    a = A()
    __dp__.setattr(a, "b", 5)
    c = object()
    __dp__.setattr(c, "a", a)
c = ff()
__dp__.delattr(c.a, "b")
__dp__.delitem(c.a.arr, 0)
del c
def _dp_gen_9(_dp_iter_10):
    _dp_iter_11 = __dp__.iter(_dp_iter_10)
    while True:
        try:
            i = __dp__.next(_dp_iter_11)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_12 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_12:
                yield __dp__.add(i, 1)
x = __dp__.list(_dp_gen_9(__dp__.iter(range(5))))
def _dp_gen_13(_dp_iter_14):
    _dp_iter_15 = __dp__.iter(_dp_iter_14)
    while True:
        try:
            i = __dp__.next(_dp_iter_15)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_16 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_16:
                yield __dp__.add(i, 1)
y = __dp__.set(_dp_gen_13(__dp__.iter(range(5))))
def _dp_gen_17(_dp_iter_18):
    _dp_iter_19 = __dp__.iter(_dp_iter_18)
    while True:
        try:
            i = __dp__.next(_dp_iter_19)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_20 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_20:
                yield __dp__.add(i, 1)
z = _dp_gen_17(__dp__.iter(range(5)))
