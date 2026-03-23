# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==

# function C.m(self):
#     function_id: 0
#     block _dp_bb_0_1:
#         return self.x

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "x", 1)
#         __dp_setitem(_dp_class_ns, "m", __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         __annotate_func__ = __dp_exec_function_def_source('def __annotate_func__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_tuple=__dp_tuple):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple(("x", "int")))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple(("x", int)))', __dp_globals(), __dp_tuple(), "__annotate_func__")
#         __dp_setitem(_dp_class_ns, "__annotate_func__", __dp_update_fn(__annotate_func__, "C.__annotate_func__", "__annotate_func__", None))
#         return __dp_NONE

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 4
#     block _dp_bb_4_1:
#         _dp_class_ns_C = __dp_make_function(2, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_C = __dp_make_function(3, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))
#         return __dp_NONE

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==

# function Wrapper.open(self, mode, *, encoding):
#     function_id: 0
#     entry_liveins: [mode, encoding]
#     block _dp_bb_0_1:
#         return open(mode, encoding=encoding)

# function _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "Wrapper")
#         _dp_fn___annotate___open = __dp_exec_function_def_source("def _dp_fn___annotate___open(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_class_lookup_global=__dp_class_lookup_global, __dp_tuple=__dp_tuple, _dp_class_ns=_dp_class_ns):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple((\"mode\", '__dp_class_lookup_global(_dp_class_ns, \"str\", globals())'), (\"encoding\", '__dp_class_lookup_global(_dp_class_ns, \"str\", globals())')))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple((\"mode\", __dp_class_lookup_global(_dp_class_ns, \"str\", globals())), (\"encoding\", __dp_class_lookup_global(_dp_class_ns, \"str\", globals()))))", __dp_globals(), __dp_tuple(("_dp_class_ns", _dp_class_ns)), "_dp_fn___annotate___open")
#         __dp_setitem(_dp_class_ns, "open", __dp_make_function(0, __dp_tuple(), __dp_tuple("r", "utf8"), __dp_globals(), _dp_fn___annotate___open))
#         return __dp_NONE

# function _dp_define_class_Wrapper(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Wrapper", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_class_ns_Wrapper = __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_Wrapper = __dp_make_function(2, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "Wrapper", _dp_define_class_Wrapper(_dp_class_ns_Wrapper, globals()))
#         return __dp_NONE

# class_with_base


class D(Base):
    pass


# ==

# function _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg):
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "D")
#         return __dp_NONE

# function _dp_define_class_D(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("D", _dp_class_ns_fn, __dp_tuple(Base), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_class_ns_D = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_D = __dp_make_function(1, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "D", _dp_define_class_D(_dp_class_ns_D, globals()))
#         return __dp_NONE

# class_scope_inner_capture


def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


# ==

# function outer.<locals>._dp_class_ns_Inner(_dp_class_ns, _dp_classcell_arg):
#     function_id: 0
#     entry_liveins: [_dp_class_ns, _dp_classcell_arg, _dp_cell_x]
#     local_cell_slots: [_dp_cell__dp_class_ns, _dp_cell__dp_classcell_arg]
#     freevars: [x->_dp_cell_x@inherited]
#     cellvars: [_dp_class_ns->_dp_cell__dp_class_ns@param, _dp_classcell_arg->_dp_cell__dp_classcell_arg@param]
#     block _dp_bb_0_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "outer.<locals>.Inner")
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "y", __dp_class_lookup_cell(_dp_class_ns, "x", _dp_cell_x))
#         return __dp_NONE

# function outer.<locals>._dp_define_class_Inner(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Inner", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 6, ())

# function outer():
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, "outer")
#         _dp_class_ns_Inner = __dp_make_function(0, __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_Inner = __dp_make_function(1, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         Inner = _dp_define_class_Inner(_dp_class_ns_Inner, globals())
#         return Inner.y

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_3_1:
#         __dp_store_global(globals(), "outer", __dp_make_function(2, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


# ==

# function X.f(x):
#     function_id: 0
#     entry_liveins: [x, _dp_classcell]
#     local_cell_slots: [_dp_cell_x]
#     freevars: [_dp_classcell->_dp_classcell@inherited]
#     cellvars: [x->_dp_cell_x@param]
#     block _dp_bb_0_1:
#         __dp_delattr(_dp_classcell, "cell_contents")
#         __dp_call_super(super, _dp_classcell, x)
#         return __dp_NONE

# function _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "X")
#         __dp_setitem(_dp_class_ns, "f", __dp_make_function(0, __dp_tuple(__dp_tuple("_dp_classcell", _dp_classcell)), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# function _dp_define_class_X(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("X", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, True, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_class_ns_X = __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_X = __dp_make_function(2, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "X", _dp_define_class_X(_dp_class_ns_X, globals()))
#         return __dp_NONE

# nested classes


class A:
    class B:
        pass


# ==

# function A._dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A.B")
#         return __dp_NONE

# function A._dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A")
#         _dp_class_ns_B = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_B = __dp_make_function(1, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "B", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))
#         return __dp_NONE

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 4
#     block _dp_bb_4_1:
#         _dp_class_ns_A = __dp_make_function(2, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_A = __dp_make_function(3, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "A", _dp_define_class_A(_dp_class_ns_A, globals()))
#         return __dp_NONE

# nested classes with weird scoping


def foo():
    class A:
        global B

        class B:
            pass


# ==

# function foo.<locals>.A._dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "B")
#         return __dp_NONE

# function foo.<locals>.A._dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 7, ())

# function foo.<locals>._dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "foo.<locals>.A")
#         _dp_class_ns_B = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_B = __dp_make_function(1, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "B", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))
#         return __dp_NONE

# function foo.<locals>._dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function foo():
#     function_id: 4
#     block _dp_bb_4_1:
#         _dp_class_ns_A = __dp_make_function(2, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_A = __dp_make_function(3, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         A = _dp_define_class_A(_dp_class_ns_A, globals())
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 5
#     block _dp_bb_5_1:
#         __dp_store_global(globals(), "foo", __dp_make_function(4, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE
