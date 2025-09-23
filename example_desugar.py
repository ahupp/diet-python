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
    _dp_tmp_6 = "A"
    __dp__.setitem(_dp_temp_ns, "__qualname__", _dp_tmp_6)
    __dp__.setitem(_dp_prepare_ns, "__qualname__", _dp_tmp_6)
    _dp_class_annotations = _dp_temp_ns.get("__annotations__")
    _dp_tmp_7 = __dp__.is_(_dp_class_annotations, None)
    if _dp_tmp_7:
        _dp_class_annotations = __dp__.dict()
    _dp_var_b_1 = 1
    __dp__.setitem(_dp_temp_ns, "b", _dp_var_b_1)
    __dp__.setitem(_dp_prepare_ns, "b", _dp_var_b_1)

    def _dp_var___init___2(self):
        __dp__.setattr(self, "arr", __dp__.list((1, 2, 3)))
    __dp__.setitem(_dp_temp_ns, "__init__", _dp_var___init___2)
    __dp__.setitem(_dp_prepare_ns, "__init__", _dp_var___init___2)

    def _dp_var_c_3(self, d):
        return add(d, 2)
    __dp__.setitem(_dp_temp_ns, "c", _dp_var_c_3)
    __dp__.setitem(_dp_prepare_ns, "c", _dp_var_c_3)

    async def _dp_var_test_aiter_4(self):
        _dp_iter_8 = __dp__.iter(range(10))
        while True:
            try:
                i = __dp__.next(_dp_iter_8)
            except:
                __dp__.check_stopiteration()
                break
            else:
                yield i
    __dp__.setitem(_dp_temp_ns, "test_aiter", _dp_var_test_aiter_4)
    __dp__.setitem(_dp_prepare_ns, "test_aiter", _dp_var_test_aiter_4)

    async def _dp_var_d_5(self):
        _dp_iter_9 = __dp__.aiter(self.test_aiter())
        while True:
            try:
                i = await __dp__.anext(_dp_iter_9)
            except:
                __dp__.acheck_stopiteration()
                break
            else:
                print(i)
    __dp__.setitem(_dp_temp_ns, "d", _dp_var_d_5)
    __dp__.setitem(_dp_prepare_ns, "d", _dp_var_d_5)
def _dp_make_class_A():
    orig_bases = ()
    bases = __dp__.resolve_bases(orig_bases)
    _dp_tmp_10 = __dp__.prepare_class("A", bases, None)
    meta = __dp__.getitem(_dp_tmp_10, 0)
    ns = __dp__.getitem(_dp_tmp_10, 1)
    kwds = __dp__.getitem(_dp_tmp_10, 2)
    _dp_ns_A(ns)
    _dp_tmp_12 = __dp__.is_not(orig_bases, bases)
    _dp_tmp_11 = _dp_tmp_12
    if _dp_tmp_11:
        _dp_tmp_13 = __dp__.not_(__dp__.contains(ns, "__orig_bases__"))
        _dp_tmp_11 = _dp_tmp_13
    if _dp_tmp_11:
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
def _dp_gen_14(_dp_iter_15):
    _dp_iter_16 = __dp__.iter(_dp_iter_15)
    while True:
        try:
            i = __dp__.next(_dp_iter_16)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_17 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_17:
                yield __dp__.add(i, 1)
x = __dp__.list(_dp_gen_14(__dp__.iter(range(5))))
def _dp_gen_18(_dp_iter_19):
    _dp_iter_20 = __dp__.iter(_dp_iter_19)
    while True:
        try:
            i = __dp__.next(_dp_iter_20)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_21 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_21:
                yield __dp__.add(i, 1)
y = __dp__.set(_dp_gen_18(__dp__.iter(range(5))))
def _dp_gen_22(_dp_iter_23):
    _dp_iter_24 = __dp__.iter(_dp_iter_23)
    while True:
        try:
            i = __dp__.next(_dp_iter_24)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_25 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_25:
                yield __dp__.add(i, 1)
z = _dp_gen_22(__dp__.iter(range(5)))
