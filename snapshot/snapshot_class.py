# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==

# module_init: _dp_module_init

# function m(self)
#     kind: function
#     bind: m
#     target: class_namespace
#     qualname: C.m
#     block _dp_bb_m_start:
#         return self.x

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_C
#     target: local
#     qualname: _dp_class_ns_C
#     block _dp_bb__dp_class_ns_C_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "x", 1)
#         __dp_setitem(_dp_class_ns, "m", __dp_def_fn("_dp_bb_m_start", "m", "C.m", __dp_tuple("self"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"self"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None))
#         __annotate_func__ = __dp_exec_function_def_source('def __annotate_func__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_tuple=__dp_tuple):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple(("x", "int")))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple(("x", int)))', __dp_globals(), __dp_tuple(), "__annotate_func__")
#         __dp_setitem(_dp_class_ns, "__annotate_func__", __dp_update_fn(__annotate_func__, "C.__annotate_func__", "__annotate_func__", None))
#         return

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_C
#     target: local
#     qualname: _dp_define_class_C
#     block _dp_bb__dp_define_class_C_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         _dp_class_ns_C = __dp_def_fn("_dp_bb__dp_class_ns_C_start", "_dp_class_ns_C", "_dp_class_ns_C", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_C = __dp_def_fn("_dp_bb__dp_define_class_C_start", "_dp_define_class_C", "_dp_define_class_C", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))
#         return

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==

# module_init: _dp_module_init

# function open(self, mode: __dp_class_lookup_global(_dp_class_ns, "str", globals()) = "r", *, encoding: __dp_class_lookup_global(_dp_class_ns, "str", globals()) = "utf8")
#     kind: function
#     bind: open
#     target: class_namespace
#     qualname: Wrapper.open
#     entry_liveins: [mode, encoding]
#     cellvars: [mode->_dp_cell_mode@param, encoding->_dp_cell_encoding@param]
#     block _dp_bb_open_start:
#         return open(mode, encoding=encoding)

# function _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_Wrapper
#     target: local
#     qualname: _dp_class_ns_Wrapper
#     cellvars: [_dp_class_ns->_dp_cell__dp_class_ns@param, _dp_classcell_arg->_dp_cell__dp_classcell_arg@param]
#     block _dp_bb__dp_class_ns_Wrapper_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "Wrapper")
#         _dp_fn___annotate___open = __dp_exec_function_def_source("def _dp_fn___annotate___open(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_class_lookup_global=__dp_class_lookup_global, __dp_tuple=__dp_tuple, _dp_class_ns=_dp_class_ns):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple((\"mode\", '__dp_class_lookup_global(_dp_class_ns, \"str\", globals())'), (\"encoding\", '__dp_class_lookup_global(_dp_class_ns, \"str\", globals())')))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple((\"mode\", __dp_class_lookup_global(_dp_class_ns, \"str\", globals())), (\"encoding\", __dp_class_lookup_global(_dp_class_ns, \"str\", globals()))))", __dp_globals(), __dp_tuple(("_dp_class_ns", _dp_class_ns)), "_dp_fn___annotate___open")
#         __dp_setitem(_dp_class_ns, "open", __dp_def_fn("_dp_bb_open_start", "open", "Wrapper.open", __dp_tuple("mode", "encoding"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"self"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"mode"), __dp_NONE, __dp_decode_literal_bytes(b"r")), __dp_tuple(__dp_decode_literal_bytes(b"kw:encoding"), __dp_NONE, __dp_decode_literal_bytes(b"utf8"))), __dp_globals(), __name__, None, _dp_fn___annotate___open))
#         return

# function _dp_define_class_Wrapper(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_Wrapper
#     target: local
#     qualname: _dp_define_class_Wrapper
#     block _dp_bb__dp_define_class_Wrapper_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Wrapper", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         _dp_class_ns_Wrapper = __dp_def_fn("_dp_bb__dp_class_ns_Wrapper_start", "_dp_class_ns_Wrapper", "_dp_class_ns_Wrapper", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_Wrapper = __dp_def_fn("_dp_bb__dp_define_class_Wrapper_start", "_dp_define_class_Wrapper", "_dp_define_class_Wrapper", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "Wrapper", _dp_define_class_Wrapper(_dp_class_ns_Wrapper, globals()))
#         return

# class_with_base


class D(Base):
    pass


# ==

# module_init: _dp_module_init

# function _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_D
#     target: local
#     qualname: _dp_class_ns_D
#     block _dp_bb__dp_class_ns_D_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "D")
#         return

# function _dp_define_class_D(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_D
#     target: local
#     qualname: _dp_define_class_D
#     block _dp_bb__dp_define_class_D_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("D", _dp_class_ns_fn, __dp_tuple(Base), _dp_prepare_dict, False, 3, ())

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         _dp_class_ns_D = __dp_def_fn("_dp_bb__dp_class_ns_D_start", "_dp_class_ns_D", "_dp_class_ns_D", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_D = __dp_def_fn("_dp_bb__dp_define_class_D_start", "_dp_define_class_D", "_dp_define_class_D", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "D", _dp_define_class_D(_dp_class_ns_D, globals()))
#         return

# class_scope_inner_capture


def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


# ==

# module_init: _dp_module_init

# function _dp_class_ns_Inner(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_Inner
#     target: local
#     qualname: outer.<locals>._dp_class_ns_Inner
#     entry_liveins: [_dp_class_ns, _dp_classcell_arg, _dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     cellvars: [_dp_class_ns->_dp_cell__dp_class_ns@param, _dp_classcell_arg->_dp_cell__dp_classcell_arg@param]
#     block _dp_bb__dp_class_ns_Inner_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "outer.<locals>.Inner")
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "y", __dp_class_lookup_cell(_dp_class_ns, "x", _dp_cell_x))
#         return

# function _dp_define_class_Inner(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_Inner
#     target: local
#     qualname: outer.<locals>._dp_define_class_Inner
#     block _dp_bb__dp_define_class_Inner_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Inner", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 6, ())

# function outer()
#     kind: function
#     bind: outer
#     target: module_global
#     qualname: outer
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block _dp_bb_outer_start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, "outer")
#         _dp_class_ns_Inner = __dp_def_fn("_dp_bb__dp_class_ns_Inner_start", "_dp_class_ns_Inner", "outer.<locals>._dp_class_ns_Inner", __dp_tuple("_dp_class_ns", "_dp_classcell_arg", __dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_Inner = __dp_def_fn("_dp_bb__dp_define_class_Inner_start", "_dp_define_class_Inner", "outer.<locals>._dp_define_class_Inner", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         Inner = _dp_define_class_Inner(_dp_class_ns_Inner, globals())
#         return Inner.y

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         __dp_store_global(globals(), "outer", __dp_def_fn("_dp_bb_outer_start", "outer", "outer", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


# ==

# module_init: _dp_module_init

# function f(x)
#     kind: function
#     bind: f
#     target: class_namespace
#     qualname: X.f
#     entry_liveins: [x, _dp_classcell]
#     freevars: [_dp_classcell->_dp_classcell@inherited]
#     cellvars: [x->_dp_cell_x@param]
#     block _dp_bb_f_start:
#         __dp_delattr(_dp_classcell, "cell_contents")
#         __dp_call_super(super, _dp_classcell, x)
#         return

# function _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_X
#     target: local
#     qualname: _dp_class_ns_X
#     block _dp_bb__dp_class_ns_X_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "X")
#         __dp_setitem(_dp_class_ns, "f", __dp_def_fn("_dp_bb_f_start", "f", "X.f", __dp_tuple("x", __dp_tuple("_dp_classcell", _dp_classcell)), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"x"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None))
#         return

# function _dp_define_class_X(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_X
#     target: local
#     qualname: _dp_define_class_X
#     block _dp_bb__dp_define_class_X_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("X", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, True, 3, ())

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         _dp_class_ns_X = __dp_def_fn("_dp_bb__dp_class_ns_X_start", "_dp_class_ns_X", "_dp_class_ns_X", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_X = __dp_def_fn("_dp_bb__dp_define_class_X_start", "_dp_define_class_X", "_dp_define_class_X", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "X", _dp_define_class_X(_dp_class_ns_X, globals()))
#         return

# nested classes


class A:
    class B:
        pass


# ==

# module_init: _dp_module_init

# function _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_B
#     target: local
#     qualname: A._dp_class_ns_B
#     block _dp_bb__dp_class_ns_B_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A.B")
#         return

# function _dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_B
#     target: local
#     qualname: A._dp_define_class_B
#     block _dp_bb__dp_define_class_B_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_A
#     target: local
#     qualname: _dp_class_ns_A
#     cellvars: [_dp_class_ns->_dp_cell__dp_class_ns@param, _dp_classcell_arg->_dp_cell__dp_classcell_arg@param]
#     block _dp_bb__dp_class_ns_A_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A")
#         _dp_class_ns_B = __dp_def_fn("_dp_bb__dp_class_ns_B_start", "_dp_class_ns_B", "A._dp_class_ns_B", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_B = __dp_def_fn("_dp_bb__dp_define_class_B_start", "_dp_define_class_B", "A._dp_define_class_B", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "B", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))
#         return

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_A
#     target: local
#     qualname: _dp_define_class_A
#     block _dp_bb__dp_define_class_A_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         _dp_class_ns_A = __dp_def_fn("_dp_bb__dp_class_ns_A_start", "_dp_class_ns_A", "_dp_class_ns_A", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_A = __dp_def_fn("_dp_bb__dp_define_class_A_start", "_dp_define_class_A", "_dp_define_class_A", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "A", _dp_define_class_A(_dp_class_ns_A, globals()))
#         return

# nested classes with weird scoping


def foo():
    class A:
        global B

        class B:
            pass


# ==

# module_init: _dp_module_init

# function _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_B
#     target: local
#     qualname: foo.<locals>.A._dp_class_ns_B
#     block _dp_bb__dp_class_ns_B_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "B")
#         return

# function _dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_B
#     target: local
#     qualname: foo.<locals>.A._dp_define_class_B
#     block _dp_bb__dp_define_class_B_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 7, ())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_A
#     target: local
#     qualname: foo.<locals>._dp_class_ns_A
#     block _dp_bb__dp_class_ns_A_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "foo.<locals>.A")
#         _dp_class_ns_B = __dp_def_fn("_dp_bb__dp_class_ns_B_start", "_dp_class_ns_B", "foo.<locals>.A._dp_class_ns_B", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_B = __dp_def_fn("_dp_bb__dp_define_class_B_start", "_dp_define_class_B", "foo.<locals>.A._dp_define_class_B", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "B", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))
#         return

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_A
#     target: local
#     qualname: foo.<locals>._dp_define_class_A
#     block _dp_bb__dp_define_class_A_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function foo()
#     kind: function
#     bind: foo
#     target: module_global
#     qualname: foo
#     block _dp_bb_foo_start:
#         _dp_class_ns_A = __dp_def_fn("_dp_bb__dp_class_ns_A_start", "_dp_class_ns_A", "foo.<locals>._dp_class_ns_A", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_classcell_arg"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT")))), __dp_globals(), __name__, None, None)
#         _dp_define_class_A = __dp_def_fn("_dp_bb__dp_define_class_A_start", "_dp_define_class_A", "foo.<locals>._dp_define_class_A", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_fn"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_class_ns_outer"), __dp_NONE, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"NO_DEFAULT"))), __dp_tuple(__dp_decode_literal_bytes(b"_dp_prepare_dict"), __dp_NONE, __dp_NONE)), __dp_globals(), __name__, None, None)
#         A = _dp_define_class_A(_dp_class_ns_A, globals())
#         return

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     target: module_global
#     qualname: _dp_module_init
#     block _dp_bb__dp_module_init_start:
#         __dp_store_global(globals(), "foo", __dp_def_fn("_dp_bb_foo_start", "foo", "foo", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return
