import __dp__
sys = __dp__.import_("sys", __spec__)
ei = __dp__.import_("sys", __spec__, __dp__.list(("exc_info",))).exc_info
_dp_tmp_1 = bar(1, 2)
def add(a, b):
    return __dp__.add(a, b)
add = foo(_dp_tmp_1(add))
def _dp_ns_A(_dp_class_ns):
    __dp__.setattr(_dp_class_ns, "b", 1)

    def __init__(self):
        __dp__.setattr(self, "arr", __dp__.list((1, 2, 3)))
    __dp__.setattr(_dp_class_ns, "__init__", __init__)

    def c(self, d):
        return add(d, 2)
    __dp__.setattr(_dp_class_ns, "c", c)

    async def test_aiter(self):
        _dp_iter_2 = __dp__.iter(range(10))
        while True:
            try:
                i = __dp__.next(_dp_iter_2)
            except:
                __dp__.check_stopiteration()
                break
            else:
                yield i
    __dp__.setattr(_dp_class_ns, "test_aiter", test_aiter)

    async def d(self):
        _dp_iter_3 = __dp__.aiter(self.test_aiter())
        while True:
            try:
                i = await __dp__.anext(_dp_iter_3)
            except:
                __dp__.acheck_stopiteration()
                break
            else:
                print(i)
    __dp__.setattr(_dp_class_ns, "d", d)
    __dp__.setattr(_dp_class_ns, "__module__", __name__)
    __dp__.setattr(_dp_class_ns, "__qualname__", "A")
A = __dp__.create_class("A", _dp_ns_A, (), None)
del _dp_ns_A
def ff():
    a = A()
    __dp__.setattr(a, "b", 5)
    c = object()
    __dp__.setattr(c, "a", a)
c = ff()
__dp__.delattr(c.a, "b")
__dp__.delitem(c.a.arr, 0)
del c
def _dp_gen_4(_dp_iter_5):
    _dp_iter_6 = __dp__.iter(_dp_iter_5)
    while True:
        try:
            i = __dp__.next(_dp_iter_6)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_7 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_7:
                yield __dp__.add(i, 1)
x = __dp__.list(_dp_gen_4(__dp__.iter(range(5))))
def _dp_gen_8(_dp_iter_9):
    _dp_iter_10 = __dp__.iter(_dp_iter_9)
    while True:
        try:
            i = __dp__.next(_dp_iter_10)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_11 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_11:
                yield __dp__.add(i, 1)
y = __dp__.set(_dp_gen_8(__dp__.iter(range(5))))
def _dp_gen_12(_dp_iter_13):
    _dp_iter_14 = __dp__.iter(_dp_iter_13)
    while True:
        try:
            i = __dp__.next(_dp_iter_14)
        except:
            __dp__.check_stopiteration()
            break
        else:
            _dp_tmp_15 = __dp__.eq(__dp__.mod(i, 2), 0)
            if _dp_tmp_15:
                yield __dp__.add(i, 1)
z = _dp_gen_12(__dp__.iter(range(5)))
