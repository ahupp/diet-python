# class_with_method

class C:
    x: int = 1

    def m(self):
        return self.x

# ==

# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "C")
        __dp__.setitem(_dp_class_ns, "x", 1)

        def m(self):
            return self.x

        def __annotate_func__(_dp_format, _dp=__dp__):
            if _dp.eq(_dp_format, 4):
                return __dp__.dict((("x", "int"),))
            if _dp.gt(_dp_format, 2):
                raise _dp.builtins.NotImplementedError
            return __dp__.dict((("x", int),))

    def _dp_define_class_C():
        return __dp__.create_class("C", _dp_class_ns_C, (), None, False, 2, ())
    __dp__.store_global(globals(), "C", _dp_define_class_C())

# -- bb --
def _dp_bb_m_start(_dp_args_ptr):
    self = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(self.x)
def _dp_bb__dp_module_init_start(_dp_args_ptr):

    def _dp_bb__dp_class_ns_C_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "C")
        __dp__.setitem(_dp_class_ns, "x", 1)
        __dp__.setitem(_dp_class_ns, "m", __dp__.def_fn(_dp_bb_m_start, "m", "C.m", ("self",), (("self", None, __dp__.NO_DEFAULT),), __name__))

        def __annotate_func__(_dp_format, _dp=__dp__):
            if _dp.eq(_dp_format, 4):
                return __dp__.dict((("x", "int"),))
            if _dp.gt(_dp_format, 2):
                raise _dp.builtins.NotImplementedError
            return __dp__.dict((("x", int),))
        __dp__.setitem(_dp_class_ns, "__annotate_func__", __dp__.update_fn(__annotate_func__, "C.__annotate_func__", "__annotate_func__"))
        return __dp__.ret(None)
    _dp_class_ns_C = __dp__.def_fn(_dp_bb__dp_class_ns_C_start, "_dp_class_ns_C", "_dp_class_ns_C", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_C_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("C", _dp_class_ns_C, (), None, False, 2, ()))
    _dp_define_class_C = __dp__.def_fn(_dp_bb__dp_define_class_C_start, "_dp_define_class_C", "_dp_define_class_C", (), (), __name__)
    __dp__.store_global(globals(), "C", _dp_define_class_C())
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)

# class_method_named_open_calls_builtin

class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)

# ==

# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "Wrapper")

        def open(self, mode: __dp__.class_lookup_global(_dp_class_ns, "str", globals())="r", *, encoding: __dp__.class_lookup_global(_dp_class_ns, "str", globals())="utf8"):
            return open(mode, encoding=encoding)

    def _dp_define_class_Wrapper():
        return __dp__.create_class("Wrapper", _dp_class_ns_Wrapper, (), None, False, 2, ())
    __dp__.store_global(globals(), "Wrapper", _dp_define_class_Wrapper())

# -- bb --
def _dp_bb_open_start(_dp_args_ptr):
    mode, encoding = __dp__.take_args(_dp_args_ptr)
    return __dp__.ret(open(mode, encoding=encoding))
def _dp_bb__dp_module_init_start(_dp_args_ptr):

    def _dp_bb__dp_class_ns_Wrapper_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "Wrapper")
        __dp__.setitem(_dp_class_ns, "open", __dp__.apply_fn_metadata(__dp__.def_fn(_dp_bb_open_start, "open", "Wrapper.open", ("mode", "encoding"), (("self", None, __dp__.NO_DEFAULT), ("mode", None, "r"), ("kw:encoding", None, "utf8")), __name__), None, (("mode", lambda: __dp__.class_lookup_global(_dp_class_ns, "str", globals()), '__dp__.class_lookup_global(_dp_class_ns, "str", globals())'), ("encoding", lambda: __dp__.class_lookup_global(_dp_class_ns, "str", globals()), '__dp__.class_lookup_global(_dp_class_ns, "str", globals())'))))
        return __dp__.ret(None)
    _dp_class_ns_Wrapper = __dp__.def_fn(_dp_bb__dp_class_ns_Wrapper_start, "_dp_class_ns_Wrapper", "_dp_class_ns_Wrapper", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_Wrapper_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("Wrapper", _dp_class_ns_Wrapper, (), None, False, 2, ()))
    _dp_define_class_Wrapper = __dp__.def_fn(_dp_bb__dp_define_class_Wrapper_start, "_dp_define_class_Wrapper", "_dp_define_class_Wrapper", (), (), __name__)
    __dp__.store_global(globals(), "Wrapper", _dp_define_class_Wrapper())
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)

# class_with_base

class D(Base):
    pass

# ==

# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "D")

    def _dp_define_class_D():
        return __dp__.create_class("D", _dp_class_ns_D, (Base,), None, False, 2, ())
    __dp__.store_global(globals(), "D", _dp_define_class_D())

# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):

    def _dp_bb__dp_class_ns_D_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "D")
        return __dp__.ret(None)
    _dp_class_ns_D = __dp__.def_fn(_dp_bb__dp_class_ns_D_start, "_dp_class_ns_D", "_dp_class_ns_D", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_D_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("D", _dp_class_ns_D, (Base,), None, False, 2, ()))
    _dp_define_class_D = __dp__.def_fn(_dp_bb__dp_define_class_D_start, "_dp_define_class_D", "_dp_define_class_D", (), (), __name__)
    __dp__.store_global(globals(), "D", _dp_define_class_D())
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)

# class_scope_inner_capture

def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y

# ==

# -- pre-bb --
def _dp_module_init():

    def outer():
        _dp_cell_x = __dp__.make_cell()
        __dp__.store_cell(_dp_cell_x, "outer")

        def _dp_class_ns_Inner(_dp_class_ns, _dp_classcell_arg):
            _dp_classcell = _dp_classcell_arg
            __dp__.setitem(_dp_class_ns, "__module__", __name__)
            __dp__.setitem(_dp_class_ns, "__qualname__", "outer.<locals>.Inner")
            __dp__.setitem(_dp_class_ns, "y", __dp__.class_lookup_cell(_dp_class_ns, "x", _dp_cell_x))

        def _dp_define_class_Inner():
            return __dp__.create_class("Inner", _dp_class_ns_Inner, (), None, False, 5, ())
        Inner = _dp_define_class_Inner()
        return Inner.y

# -- bb --
def _dp_bb_outer_start(_dp_args_ptr):
    _dp_cell_x = __dp__.make_cell()
    __dp__.store_cell(_dp_cell_x, "outer")

    def _dp_bb__dp_class_ns_Inner_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg, _dp_cell_x = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "outer.<locals>.Inner")
        __dp__.setitem(_dp_class_ns, "y", __dp__.class_lookup_cell(_dp_class_ns, "x", _dp_cell_x))
        return __dp__.ret(None)
    _dp_class_ns_Inner = __dp__.def_fn(_dp_bb__dp_class_ns_Inner_start, "_dp_class_ns_Inner", "_dp_class_ns_Inner", ("_dp_class_ns", "_dp_classcell_arg", ("_dp_cell_x", _dp_cell_x)), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_Inner_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("Inner", _dp_class_ns_Inner, (), None, False, 5, ()))
    _dp_define_class_Inner = __dp__.def_fn(_dp_bb__dp_define_class_Inner_start, "_dp_define_class_Inner", "_dp_define_class_Inner", (), (), __name__)
    Inner = _dp_define_class_Inner()
    return __dp__.ret(Inner.y)
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "outer", __dp__.def_fn(_dp_bb_outer_start, "outer", "outer", (), (), __name__))
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)

# class_super_empty_classcell

class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()

# ==

# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "X")

        def f(x):
            __dp__.delattr(_dp_classcell, "cell_contents")
            __dp__.call_super(super, _dp_classcell, x)

    def _dp_define_class_X():
        return __dp__.create_class("X", _dp_class_ns_X, (), None, True, 2, ())
    __dp__.store_global(globals(), "X", _dp_define_class_X())

# -- bb --
def _dp_bb_f_start(_dp_args_ptr):
    x, _dp_classcell = __dp__.take_args(_dp_args_ptr)
    __dp__.delattr(_dp_classcell, "cell_contents")
    __dp__.call_super(super, _dp_classcell, x)
    return __dp__.ret(None)
def _dp_bb__dp_module_init_start(_dp_args_ptr):

    def _dp_bb__dp_class_ns_X_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "X")
        __dp__.setitem(_dp_class_ns, "f", __dp__.def_fn(_dp_bb_f_start, "f", "X.f", ("x", ("_dp_classcell", _dp_classcell)), (("x", None, __dp__.NO_DEFAULT),), __name__))
        return __dp__.ret(None)
    _dp_class_ns_X = __dp__.def_fn(_dp_bb__dp_class_ns_X_start, "_dp_class_ns_X", "_dp_class_ns_X", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_X_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("X", _dp_class_ns_X, (), None, True, 2, ()))
    _dp_define_class_X = __dp__.def_fn(_dp_bb__dp_define_class_X_start, "_dp_define_class_X", "_dp_define_class_X", (), (), __name__)
    __dp__.store_global(globals(), "X", _dp_define_class_X())
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)

# nested classes

class A:
    class B:
        pass

# ==

# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "A.B")

    def _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "A")

        def _dp_define_class_B():
            return __dp__.create_class("B", _dp_class_ns_B, (), None, False, 3, ())
        __dp__.setitem(_dp_class_ns, "B", _dp_define_class_B())

    def _dp_define_class_A():
        return __dp__.create_class("A", _dp_class_ns_A, (), None, False, 2, ())
    __dp__.store_global(globals(), "A", _dp_define_class_A())

# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):

    def _dp_bb__dp_class_ns_B_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "A.B")
        return __dp__.ret(None)
    _dp_class_ns_B = __dp__.def_fn(_dp_bb__dp_class_ns_B_start, "_dp_class_ns_B", "_dp_class_ns_B", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_class_ns_A_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "A")

        def _dp_bb__dp_define_class_B_start(_dp_args_ptr):
            return __dp__.ret(__dp__.create_class("B", _dp_class_ns_B, (), None, False, 3, ()))
        _dp_define_class_B = __dp__.def_fn(_dp_bb__dp_define_class_B_start, "_dp_define_class_B", "_dp_define_class_B", (), (), __name__)
        __dp__.setitem(_dp_class_ns, "B", _dp_define_class_B())
        return __dp__.ret(None)
    _dp_class_ns_A = __dp__.def_fn(_dp_bb__dp_class_ns_A_start, "_dp_class_ns_A", "_dp_class_ns_A", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_A_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("A", _dp_class_ns_A, (), None, False, 2, ()))
    _dp_define_class_A = __dp__.def_fn(_dp_bb__dp_define_class_A_start, "_dp_define_class_A", "_dp_define_class_A", (), (), __name__)
    __dp__.store_global(globals(), "A", _dp_define_class_A())
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)

# nested classes with weird scoping

def foo():
    class A:
        global B

        class B:
            pass

# ==

# -- pre-bb --
def _dp_module_init():

    def foo():

        def _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
            _dp_classcell = _dp_classcell_arg
            __dp__.setitem(_dp_class_ns, "__module__", __name__)
            __dp__.setitem(_dp_class_ns, "__qualname__", "B")

        def _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
            _dp_classcell = _dp_classcell_arg
            __dp__.setitem(_dp_class_ns, "__module__", __name__)
            __dp__.setitem(_dp_class_ns, "__qualname__", "foo.<locals>.A")

            def _dp_define_class_B():
                return __dp__.create_class("B", _dp_class_ns_B, (), None, False, 6, ())
            __dp__.store_global(globals(), "B", _dp_define_class_B())

        def _dp_define_class_A():
            return __dp__.create_class("A", _dp_class_ns_A, (), None, False, 3, ())
        A = _dp_define_class_A()

# -- bb --
def _dp_bb_foo_start(_dp_args_ptr):

    def _dp_bb__dp_class_ns_B_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "B")
        return __dp__.ret(None)
    _dp_class_ns_B = __dp__.def_fn(_dp_bb__dp_class_ns_B_start, "_dp_class_ns_B", "_dp_class_ns_B", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_class_ns_A_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "foo.<locals>.A")

        def _dp_bb__dp_define_class_B_start(_dp_args_ptr):
            return __dp__.ret(__dp__.create_class("B", _dp_class_ns_B, (), None, False, 6, ()))
        _dp_define_class_B = __dp__.def_fn(_dp_bb__dp_define_class_B_start, "_dp_define_class_B", "_dp_define_class_B", (), (), __name__)
        __dp__.store_global(globals(), "B", _dp_define_class_B())
        return __dp__.ret(None)
    _dp_class_ns_A = __dp__.def_fn(_dp_bb__dp_class_ns_A_start, "_dp_class_ns_A", "_dp_class_ns_A", ("_dp_class_ns", "_dp_classcell_arg"), (("_dp_class_ns", None, __dp__.NO_DEFAULT), ("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __name__)

    def _dp_bb__dp_define_class_A_start(_dp_args_ptr):
        return __dp__.ret(__dp__.create_class("A", _dp_class_ns_A, (), None, False, 3, ()))
    _dp_define_class_A = __dp__.def_fn(_dp_bb__dp_define_class_A_start, "_dp_define_class_A", "_dp_define_class_A", (), (), __name__)
    A = _dp_define_class_A()
    return __dp__.ret(None)
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "foo", __dp__.def_fn(_dp_bb_foo_start, "foo", "foo", (), (), __name__))
    return __dp__.ret(None)
_dp_module_init = __dp__.def_fn(_dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__)
