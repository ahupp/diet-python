def _dp_gen_1(_dp_iter_1):
    _dp_tmp_1 = getattr(dp_intrinsics, "iter")
    _dp_tmp_2 = _dp_tmp_1(_dp_iter_1)
    _dp_iter_1 = _dp_tmp_2
    while True:
        try:
            _dp_tmp_3 = getattr(dp_intrinsics, "next")
            _dp_tmp_4 = _dp_tmp_3(_dp_iter_1)
            i = _dp_tmp_4
        except StopIteration:
            break
        _dp_tmp_5 = getattr(dp_intrinsics, "operator")
        _dp_tmp_6 = getattr(_dp_tmp_5, "eq")
        _dp_tmp_7 = getattr(dp_intrinsics, "operator")
        _dp_tmp_8 = getattr(_dp_tmp_7, "mod")
        _dp_tmp_9 = _dp_tmp_8(i, 2)
        _dp_tmp_10 = _dp_tmp_6(_dp_tmp_9, 0)
        if _dp_tmp_10:
            _dp_tmp_11 = getattr(dp_intrinsics, "operator")
            _dp_tmp_12 = getattr(_dp_tmp_11, "add")
            _dp_tmp_13 = _dp_tmp_12(i, 1)
            _dp_tmp_14 = yield _dp_tmp_13
            _dp_tmp_14
def _dp_gen_2(_dp_iter_2):
    _dp_tmp_15 = getattr(dp_intrinsics, "iter")
    _dp_tmp_16 = _dp_tmp_15(_dp_iter_2)
    _dp_iter_2 = _dp_tmp_16
    while True:
        try:
            _dp_tmp_17 = getattr(dp_intrinsics, "next")
            _dp_tmp_18 = _dp_tmp_17(_dp_iter_2)
            i = _dp_tmp_18
        except StopIteration:
            break
        _dp_tmp_19 = getattr(dp_intrinsics, "operator")
        _dp_tmp_20 = getattr(_dp_tmp_19, "eq")
        _dp_tmp_21 = getattr(dp_intrinsics, "operator")
        _dp_tmp_22 = getattr(_dp_tmp_21, "mod")
        _dp_tmp_23 = _dp_tmp_22(i, 2)
        _dp_tmp_24 = _dp_tmp_20(_dp_tmp_23, 0)
        if _dp_tmp_24:
            _dp_tmp_25 = getattr(dp_intrinsics, "operator")
            _dp_tmp_26 = getattr(_dp_tmp_25, "add")
            _dp_tmp_27 = _dp_tmp_26(i, 1)
            _dp_tmp_28 = yield _dp_tmp_27
            _dp_tmp_28
def _dp_gen_3(_dp_iter_3):
    _dp_tmp_29 = getattr(dp_intrinsics, "iter")
    _dp_tmp_30 = _dp_tmp_29(_dp_iter_3)
    _dp_iter_3 = _dp_tmp_30
    while True:
        try:
            _dp_tmp_31 = getattr(dp_intrinsics, "next")
            _dp_tmp_32 = _dp_tmp_31(_dp_iter_3)
            i = _dp_tmp_32
        except StopIteration:
            break
        _dp_tmp_33 = getattr(dp_intrinsics, "operator")
        _dp_tmp_34 = getattr(_dp_tmp_33, "eq")
        _dp_tmp_35 = getattr(dp_intrinsics, "operator")
        _dp_tmp_36 = getattr(_dp_tmp_35, "mod")
        _dp_tmp_37 = _dp_tmp_36(i, 2)
        _dp_tmp_38 = _dp_tmp_34(_dp_tmp_37, 0)
        if _dp_tmp_38:
            _dp_tmp_39 = getattr(dp_intrinsics, "operator")
            _dp_tmp_40 = getattr(_dp_tmp_39, "add")
            _dp_tmp_41 = _dp_tmp_40(i, 1)
            _dp_tmp_42 = yield _dp_tmp_41
            _dp_tmp_42
import dp_intrinsics
sys = dp_intrinsics.import_("sys", __spec__)
ei = dp_intrinsics.import_("sys", __spec__, ["exc_info"]).exc_info
_dp_dec_1 = foo
_dp_tmp_43 = bar(1, 2)
_dp_dec_2 = _dp_tmp_43
def add(a, b):
    _dp_tmp_44 = getattr(dp_intrinsics, "operator")
    _dp_tmp_45 = getattr(_dp_tmp_44, "add")
    _dp_tmp_46 = _dp_tmp_45(a, b)
    return _dp_tmp_46
_dp_tmp_47 = _dp_dec_2(add)
_dp_tmp_48 = _dp_dec_1(_dp_tmp_47)
add = _dp_tmp_48
def _dp_ns_A(_ns):
    _dp_tmp_49 = ()
    _dp_tmp_50 = dict(_dp_tmp_49)
    _dp_temp_ns = _dp_tmp_50
    _dp_temp_ns["__module__"] = _ns["__module__"] = __name__
    _dp_temp_ns["__qualname__"] = _ns["__qualname__"] = "A"
    _dp_temp_ns["b"] = _ns["b"] = 1

    def _mk___init__():

        def __init__(self):
            _dp_tmp_51 = 1, 2, 3
            _dp_tmp_52 = list(_dp_tmp_51)
            self.arr = _dp_tmp_52
        _dp_tmp_53 = _ns["__qualname__"]
        _dp_tmp_54 = _dp_tmp_53 + ".__init__"
        __init__.__qualname__ = _dp_tmp_54
        return __init__
    _dp_tmp_55 = _mk___init__()
    _dp_temp_ns["__init__"] = _ns["__init__"] = _dp_tmp_55

    def _mk_c():

        def c(self, d):
            _dp_tmp_56 = add(d, 2)
            return _dp_tmp_56
        _dp_tmp_57 = _ns["__qualname__"]
        _dp_tmp_58 = _dp_tmp_57 + ".c"
        c.__qualname__ = _dp_tmp_58
        return c
    _dp_tmp_59 = _mk_c()
    _dp_temp_ns["c"] = _ns["c"] = _dp_tmp_59

    def _mk_test_aiter():

        async def test_aiter(self):
            _dp_tmp_60 = getattr(dp_intrinsics, "iter")
            _dp_tmp_61 = range(10)
            _dp_tmp_62 = _dp_tmp_60(_dp_tmp_61)
            _dp_iter_4 = _dp_tmp_62
            while True:
                try:
                    _dp_tmp_63 = getattr(dp_intrinsics, "next")
                    _dp_tmp_64 = _dp_tmp_63(_dp_iter_4)
                    i = _dp_tmp_64
                except StopIteration:
                    break
                _dp_tmp_65 = yield i
                _dp_tmp_65
        _dp_tmp_66 = _ns["__qualname__"]
        _dp_tmp_67 = _dp_tmp_66 + ".test_aiter"
        test_aiter.__qualname__ = _dp_tmp_67
        return test_aiter
    _dp_tmp_68 = _mk_test_aiter()
    _dp_temp_ns["test_aiter"] = _ns["test_aiter"] = _dp_tmp_68

    def _mk_d():

        async def d(self):
            _dp_tmp_69 = getattr(dp_intrinsics, "aiter")
            _dp_tmp_70 = getattr(self, "test_aiter")
            _dp_tmp_71 = _dp_tmp_70()
            _dp_tmp_72 = _dp_tmp_69(_dp_tmp_71)
            _dp_iter_5 = _dp_tmp_72
            while True:
                try:
                    _dp_tmp_73 = getattr(dp_intrinsics, "anext")
                    _dp_tmp_74 = _dp_tmp_73(_dp_iter_5)
                    _dp_tmp_75 = await _dp_tmp_74
                    i = _dp_tmp_75
                except StopAsyncIteration:
                    break
                _dp_tmp_76 = print(i)
                _dp_tmp_76
        _dp_tmp_77 = _ns["__qualname__"]
        _dp_tmp_78 = _dp_tmp_77 + ".d"
        d.__qualname__ = _dp_tmp_78
        return d
    _dp_tmp_79 = _mk_d()
    _dp_temp_ns["d"] = _ns["d"] = _dp_tmp_79
def _class_A():
    _dp_tmp_80 = getattr(dp_intrinsics, "resolve_bases")
    _dp_tmp_81 = ()
    _dp_tmp_82 = _dp_tmp_80(_dp_tmp_81)
    bases = _dp_tmp_82
    _dp_tmp_83 = getattr(dp_intrinsics, "prepare_class")
    _dp_tmp_84 = _dp_tmp_83("A", bases)
    meta, ns, kwds = _dp_tmp_84
    _dp_tmp_85 = _dp_ns_A(ns)
    _dp_tmp_85
    _dp_tmp_86 = meta("A", bases, ns)
    cls = _dp_tmp_86
    return cls
_dp_tmp_87 = _class_A()
A = _dp_tmp_87
def ff():
    _dp_tmp_88 = A()
    a = _dp_tmp_88
    a.b = 5
    _dp_tmp_89 = object()
    c = _dp_tmp_89
    c.a = a
_dp_tmp_90 = ff()
c = _dp_tmp_90
_dp_tmp_91 = getattr(c, "a")
del _dp_tmp_91.b
_dp_tmp_92 = getattr(dp_intrinsics, "operator")
_dp_tmp_93 = getattr(_dp_tmp_92, "delitem")
_dp_tmp_94 = getattr(c, "a")
_dp_tmp_95 = getattr(_dp_tmp_94, "arr")
_dp_tmp_96 = _dp_tmp_93(_dp_tmp_95, 0)
_dp_tmp_96
del c
_dp_tmp_97 = getattr(dp_intrinsics, "iter")
_dp_tmp_98 = range(5)
_dp_tmp_99 = _dp_tmp_97(_dp_tmp_98)
_dp_tmp_100 = _dp_gen_1(_dp_tmp_99)
_dp_tmp_101 = list(_dp_tmp_100)
x = _dp_tmp_101
_dp_tmp_102 = getattr(dp_intrinsics, "iter")
_dp_tmp_103 = range(5)
_dp_tmp_104 = _dp_tmp_102(_dp_tmp_103)
_dp_tmp_105 = _dp_gen_2(_dp_tmp_104)
_dp_tmp_106 = set(_dp_tmp_105)
y = _dp_tmp_106
_dp_tmp_107 = getattr(dp_intrinsics, "iter")
_dp_tmp_108 = range(5)
_dp_tmp_109 = _dp_tmp_107(_dp_tmp_108)
_dp_tmp_110 = _dp_gen_3(_dp_tmp_109)
z = _dp_tmp_110
