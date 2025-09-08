import dp_intrinsics
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
        _dp_tmp_5 = getattr(dp_intrinsics, "eq")
        _dp_tmp_6 = getattr(dp_intrinsics, "mod")
        _dp_tmp_7 = _dp_tmp_6(i, 2)
        _dp_tmp_8 = _dp_tmp_5(_dp_tmp_7, 0)
        if _dp_tmp_8:
            _dp_tmp_9 = getattr(dp_intrinsics, "add")
            _dp_tmp_10 = _dp_tmp_9(i, 1)
            _dp_tmp_11 = yield _dp_tmp_10
            _dp_tmp_11
def _dp_gen_2(_dp_iter_2):
    _dp_tmp_12 = getattr(dp_intrinsics, "iter")
    _dp_tmp_13 = _dp_tmp_12(_dp_iter_2)
    _dp_iter_2 = _dp_tmp_13
    while True:
        try:
            _dp_tmp_14 = getattr(dp_intrinsics, "next")
            _dp_tmp_15 = _dp_tmp_14(_dp_iter_2)
            i = _dp_tmp_15
        except StopIteration:
            break
        _dp_tmp_16 = getattr(dp_intrinsics, "eq")
        _dp_tmp_17 = getattr(dp_intrinsics, "mod")
        _dp_tmp_18 = _dp_tmp_17(i, 2)
        _dp_tmp_19 = _dp_tmp_16(_dp_tmp_18, 0)
        if _dp_tmp_19:
            _dp_tmp_20 = getattr(dp_intrinsics, "add")
            _dp_tmp_21 = _dp_tmp_20(i, 1)
            _dp_tmp_22 = yield _dp_tmp_21
            _dp_tmp_22
def _dp_gen_3(_dp_iter_3):
    _dp_tmp_23 = getattr(dp_intrinsics, "iter")
    _dp_tmp_24 = _dp_tmp_23(_dp_iter_3)
    _dp_iter_3 = _dp_tmp_24
    while True:
        try:
            _dp_tmp_25 = getattr(dp_intrinsics, "next")
            _dp_tmp_26 = _dp_tmp_25(_dp_iter_3)
            i = _dp_tmp_26
        except StopIteration:
            break
        _dp_tmp_27 = getattr(dp_intrinsics, "eq")
        _dp_tmp_28 = getattr(dp_intrinsics, "mod")
        _dp_tmp_29 = _dp_tmp_28(i, 2)
        _dp_tmp_30 = _dp_tmp_27(_dp_tmp_29, 0)
        if _dp_tmp_30:
            _dp_tmp_31 = getattr(dp_intrinsics, "add")
            _dp_tmp_32 = _dp_tmp_31(i, 1)
            _dp_tmp_33 = yield _dp_tmp_32
            _dp_tmp_33
sys = dp_intrinsics.import_("sys", __spec__)
ei = dp_intrinsics.import_("sys", __spec__, ["exc_info"]).exc_info
_dp_tmp_34 = bar(1, 2)
_dp_dec_1 = _dp_tmp_34
def add(a, b):
    _dp_tmp_35 = getattr(dp_intrinsics, "add")
    _dp_tmp_36 = _dp_tmp_35(a, b)
    return _dp_tmp_36
_dp_tmp_37 = _dp_dec_1(add)
_dp_tmp_38 = foo(_dp_tmp_37)
add = _dp_tmp_38
def _dp_ns_A(_ns):
    _dp_tmp_39 = ()
    _dp_tmp_40 = dict(_dp_tmp_39)
    _dp_temp_ns = _dp_tmp_40
    _dp_tmp_1 = __name__
    _dp_tmp_41 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_42 = _dp_tmp_41(_dp_temp_ns, "__module__", _dp_tmp_1)
    _dp_tmp_42
    _dp_tmp_43 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_44 = _dp_tmp_43(_ns, "__module__", _dp_tmp_1)
    _dp_tmp_44
    _dp_tmp_2 = "A"
    _dp_tmp_45 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_46 = _dp_tmp_45(_dp_temp_ns, "__qualname__", _dp_tmp_2)
    _dp_tmp_46
    _dp_tmp_47 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_48 = _dp_tmp_47(_ns, "__qualname__", _dp_tmp_2)
    _dp_tmp_48
    _dp_tmp_3 = 1
    _dp_tmp_49 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_50 = _dp_tmp_49(_dp_temp_ns, "b", _dp_tmp_3)
    _dp_tmp_50
    _dp_tmp_51 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_52 = _dp_tmp_51(_ns, "b", _dp_tmp_3)
    _dp_tmp_52

    def _mk___init__():

        def __init__(self):
            _dp_tmp_53 = 1, 2, 3
            _dp_tmp_54 = list(_dp_tmp_53)
            self.arr = _dp_tmp_54
        _dp_tmp_55 = getattr(dp_intrinsics, "add")
        _dp_tmp_56 = getattr(dp_intrinsics, "getitem")
        _dp_tmp_57 = _dp_tmp_56(_ns, "__qualname__")
        _dp_tmp_58 = _dp_tmp_55(_dp_tmp_57, ".__init__")
        __init__.__qualname__ = _dp_tmp_58
        return __init__
    _dp_tmp_59 = _mk___init__()
    _dp_tmp_4 = _dp_tmp_59
    _dp_tmp_60 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_61 = _dp_tmp_60(_dp_temp_ns, "__init__", _dp_tmp_4)
    _dp_tmp_61
    _dp_tmp_62 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_63 = _dp_tmp_62(_ns, "__init__", _dp_tmp_4)
    _dp_tmp_63

    def _mk_c():

        def c(self, d):
            _dp_tmp_64 = add(d, 2)
            return _dp_tmp_64
        _dp_tmp_65 = getattr(dp_intrinsics, "add")
        _dp_tmp_66 = getattr(dp_intrinsics, "getitem")
        _dp_tmp_67 = _dp_tmp_66(_ns, "__qualname__")
        _dp_tmp_68 = _dp_tmp_65(_dp_tmp_67, ".c")
        c.__qualname__ = _dp_tmp_68
        return c
    _dp_tmp_69 = _mk_c()
    _dp_tmp_5 = _dp_tmp_69
    _dp_tmp_70 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_71 = _dp_tmp_70(_dp_temp_ns, "c", _dp_tmp_5)
    _dp_tmp_71
    _dp_tmp_72 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_73 = _dp_tmp_72(_ns, "c", _dp_tmp_5)
    _dp_tmp_73

    def _mk_test_aiter():

        async def test_aiter(self):
            _dp_tmp_74 = getattr(dp_intrinsics, "iter")
            _dp_tmp_75 = range(10)
            _dp_tmp_76 = _dp_tmp_74(_dp_tmp_75)
            _dp_iter_4 = _dp_tmp_76
            while True:
                try:
                    _dp_tmp_77 = getattr(dp_intrinsics, "next")
                    _dp_tmp_78 = _dp_tmp_77(_dp_iter_4)
                    i = _dp_tmp_78
                except StopIteration:
                    break
                _dp_tmp_79 = yield i
                _dp_tmp_79
        _dp_tmp_80 = getattr(dp_intrinsics, "add")
        _dp_tmp_81 = getattr(dp_intrinsics, "getitem")
        _dp_tmp_82 = _dp_tmp_81(_ns, "__qualname__")
        _dp_tmp_83 = _dp_tmp_80(_dp_tmp_82, ".test_aiter")
        test_aiter.__qualname__ = _dp_tmp_83
        return test_aiter
    _dp_tmp_84 = _mk_test_aiter()
    _dp_tmp_6 = _dp_tmp_84
    _dp_tmp_85 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_86 = _dp_tmp_85(_dp_temp_ns, "test_aiter", _dp_tmp_6)
    _dp_tmp_86
    _dp_tmp_87 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_88 = _dp_tmp_87(_ns, "test_aiter", _dp_tmp_6)
    _dp_tmp_88

    def _mk_d():

        async def d(self):
            _dp_tmp_89 = getattr(dp_intrinsics, "aiter")
            _dp_tmp_90 = getattr(self, "test_aiter")
            _dp_tmp_91 = _dp_tmp_90()
            _dp_tmp_92 = _dp_tmp_89(_dp_tmp_91)
            _dp_iter_5 = _dp_tmp_92
            while True:
                try:
                    _dp_tmp_93 = getattr(dp_intrinsics, "anext")
                    _dp_tmp_94 = _dp_tmp_93(_dp_iter_5)
                    _dp_tmp_95 = await _dp_tmp_94
                    i = _dp_tmp_95
                except StopAsyncIteration:
                    break
                _dp_tmp_96 = print(i)
                _dp_tmp_96
        _dp_tmp_97 = getattr(dp_intrinsics, "add")
        _dp_tmp_98 = getattr(dp_intrinsics, "getitem")
        _dp_tmp_99 = _dp_tmp_98(_ns, "__qualname__")
        _dp_tmp_100 = _dp_tmp_97(_dp_tmp_99, ".d")
        d.__qualname__ = _dp_tmp_100
        return d
    _dp_tmp_101 = _mk_d()
    _dp_tmp_7 = _dp_tmp_101
    _dp_tmp_102 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_103 = _dp_tmp_102(_dp_temp_ns, "d", _dp_tmp_7)
    _dp_tmp_103
    _dp_tmp_104 = getattr(dp_intrinsics, "setitem")
    _dp_tmp_105 = _dp_tmp_104(_ns, "d", _dp_tmp_7)
    _dp_tmp_105
def _class_A():
    _dp_tmp_106 = getattr(dp_intrinsics, "resolve_bases")
    _dp_tmp_107 = ()
    _dp_tmp_108 = _dp_tmp_106(_dp_tmp_107)
    bases = _dp_tmp_108
    _dp_tmp_109 = getattr(dp_intrinsics, "prepare_class")
    _dp_tmp_110 = _dp_tmp_109("A", bases)
    meta, ns, kwds = _dp_tmp_110
    _dp_tmp_111 = _dp_ns_A(ns)
    _dp_tmp_111
    _dp_tmp_112 = meta("A", bases, ns)
    cls = _dp_tmp_112
    return cls
_dp_tmp_113 = _class_A()
A = _dp_tmp_113
def ff():
    _dp_tmp_114 = A()
    a = _dp_tmp_114
    a.b = 5
    _dp_tmp_115 = object()
    c = _dp_tmp_115
    c.a = a
_dp_tmp_116 = ff()
c = _dp_tmp_116
_dp_tmp_117 = getattr(c, "a")
del _dp_tmp_117.b
_dp_tmp_118 = getattr(dp_intrinsics, "delitem")
_dp_tmp_119 = getattr(c, "a")
_dp_tmp_120 = getattr(_dp_tmp_119, "arr")
_dp_tmp_121 = _dp_tmp_118(_dp_tmp_120, 0)
_dp_tmp_121
del c
_dp_tmp_122 = getattr(dp_intrinsics, "iter")
_dp_tmp_123 = range(5)
_dp_tmp_124 = _dp_tmp_122(_dp_tmp_123)
_dp_tmp_125 = _dp_gen_1(_dp_tmp_124)
_dp_tmp_126 = list(_dp_tmp_125)
x = _dp_tmp_126
_dp_tmp_127 = getattr(dp_intrinsics, "iter")
_dp_tmp_128 = range(5)
_dp_tmp_129 = _dp_tmp_127(_dp_tmp_128)
_dp_tmp_130 = _dp_gen_2(_dp_tmp_129)
_dp_tmp_131 = set(_dp_tmp_130)
y = _dp_tmp_131
_dp_tmp_132 = getattr(dp_intrinsics, "iter")
_dp_tmp_133 = range(5)
_dp_tmp_134 = _dp_tmp_132(_dp_tmp_133)
_dp_tmp_135 = _dp_gen_3(_dp_tmp_134)
z = _dp_tmp_135
