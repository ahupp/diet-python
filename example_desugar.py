import __dp__
sys = __dp__.import_("sys", __spec__)
ei = __dp__.import_("sys", __spec__, __dp__.list(("exc_info",))).exc_info
_dp_decorator_add_0 = foo
_dp_decorator_add_1 = bar(1, 2)
def add(a, b):
    return __dp__.add(a, b)
add = _dp_decorator_add_0(_dp_decorator_add_1(add))
def _dp_meth_A___init___1(self):
    __dp__.setattr(self, "arr", __dp__.list((1, 2, 3)))
def _dp_meth_A_c_2(self, d):
    return add(d, 2)
async def _dp_meth_A_test_aiter_3(self):
    _dp_iter_5 = __dp__.iter(range(10))
    while True:
        try:
            i = __dp__.next(_dp_iter_5)
        except:
            __dp__.check_stopiteration()
            break
        else:
            yield i
async def _dp_meth_A_d_4(self):
    _dp_iter_6 = __dp__.aiter(self.test_aiter())
    while True:
        try:
            i = await __dp__.anext(_dp_iter_6)
        except:
            __dp__.acheck_stopiteration()
            break
        else:
            print(i)
def _dp_ns_A(_dp_prepare_ns):
    _dp_temp_ns = __dp__.dict()
    __dp__.setitem(_dp_temp_ns, "__module__", __name__)
    __dp__.setitem(_dp_prepare_ns, "__module__", __name__)
    _dp_tmp_7 = "A"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_7)
    __dp__.setitem(_dp_prepare_ns, "__qualname__", _dp_tmp_7)
    _dp_class_annotations = _dp_temp_ns.get("__annotations__")
    _dp_tmp_8 = __dp__.is_(_dp_class_annotations, None)
    if _dp_tmp_8:
        _dp_class_annotations = __dp__.dict()
    b = 1
    __dp__.setitem(_dp_temp_ns, "b", b)
    __dp__.setitem(_dp_prepare_ns, "b", b)

    def __init__(self):
        pass
    __dp__.setattr(__init__, "__code__", _dp_meth_A___init___1.__code__)
    __dp__.setattr(__init__, "__doc__", _dp_meth_A___init___1.__doc__)
    __dp__.setattr(__init__, "__annotations__", _dp_meth_A___init___1.__annotations__)
    __dp__.setattr(__init__, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".__init__"))
    __dp__.setitem(_dp_temp_ns, "__init__", __init__)
    __dp__.setitem(_dp_prepare_ns, "__init__", __init__)

    def c(self, d):
        pass
    __dp__.setattr(c, "__code__", _dp_meth_A_c_2.__code__)
    __dp__.setattr(c, "__doc__", _dp_meth_A_c_2.__doc__)
    __dp__.setattr(c, "__annotations__", _dp_meth_A_c_2.__annotations__)
    __dp__.setattr(c, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".c"))
    __dp__.setitem(_dp_temp_ns, "c", c)
    __dp__.setitem(_dp_prepare_ns, "c", c)

    async def test_aiter(self):
        pass
    __dp__.setattr(test_aiter, "__code__", _dp_meth_A_test_aiter_3.__code__)
    __dp__.setattr(test_aiter, "__doc__", _dp_meth_A_test_aiter_3.__doc__)
    __dp__.setattr(test_aiter, "__annotations__", _dp_meth_A_test_aiter_3.__annotations__)
    __dp__.setattr(test_aiter, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".test_aiter"))
    __dp__.setitem(_dp_temp_ns, "test_aiter", test_aiter)
    __dp__.setitem(_dp_prepare_ns, "test_aiter", test_aiter)

    async def d(self):
        pass
    __dp__.setattr(d, "__code__", _dp_meth_A_d_4.__code__)
    __dp__.setattr(d, "__doc__", _dp_meth_A_d_4.__doc__)
    __dp__.setattr(d, "__annotations__", _dp_meth_A_d_4.__annotations__)
    __dp__.setattr(d, "__qualname__", __dp__.add(__dp__.getitem(_dp_prepare_ns, "__qualname__"), ".d"))
    __dp__.setitem(_dp_temp_ns, "d", d)
    __dp__.setitem(_dp_prepare_ns, "d", d)
def _dp_make_class_A():
    orig_bases = ()
    bases = __dp__.resolve_bases(orig_bases)
    _dp_tmp_9 = __dp__.prepare_class("A", bases, None)
    meta = __dp__.getitem(_dp_tmp_9, 0)
    ns = __dp__.getitem(_dp_tmp_9, 1)
    kwds = __dp__.getitem(_dp_tmp_9, 2)
    _dp_ns_A(ns)
    _dp_tmp_11 = __dp__.is_not(orig_bases, bases)
    _dp_tmp_10 = _dp_tmp_11
    if _dp_tmp_10:
        _dp_tmp_12 = __dp__.not_(__dp__.contains(ns, "__orig_bases__"))
        _dp_tmp_10 = _dp_tmp_12
    if _dp_tmp_10:
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
x = __dp__.list(_dp_gen_13(__dp__.iter(range(5))))
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
y = __dp__.set(_dp_gen_17(__dp__.iter(range(5))))
def _dp_gen_21(_dp_iter_22):
    _dp_iter_23 = __dp__.iter(_dp_iter_22)
    while True:
        try:
            i = __dp__.next(_dp_iter_23)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_24 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_24:
                yield __dp__.add(i, 1)
z = _dp_gen_21(__dp__.iter(range(5)))
