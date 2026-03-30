# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==

# function C.m(self):
#     function_id: 0
#     block bb1:
#         return self.x

# function C.__annotate_func__(_dp_format, _dp):
#     function_id: 1
#     block bb1:
#         if_term _dp.eq(_dp_format, 4):
#             then:
#                 block bb5:
#                     return _dp.dict(__dp_tuple(("x", "int")))
#             else:
#                 block bb2:
#                     if_term _dp.gt(_dp_format, 2):
#                         then:
#                             block bb4:
#                                 raise _dp.builtins.NotImplementedError
#                         else:
#                             block bb3:
#                                 return _dp.dict(__dp_tuple(("x", int)))

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         x = 1
#         m = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __annotate_func__ = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(runtime), __dp_globals(), None)
#         return __dp_NONE

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 4
#     block bb1:
#         _dp_class_ns_C = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_C = __dp_make_function(3, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         C = _dp_define_class_C(_dp_class_ns_C, globals())
#         return __dp_NONE

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==

# function Wrapper.open(self, mode, *, encoding):
#     function_id: 0
#     block bb1:
#         return open(mode, encoding=encoding)

# function _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "Wrapper")
#         open = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple("r", "utf8"), __dp_globals(), None)
#         return __dp_NONE

# function _dp_define_class_Wrapper(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Wrapper", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         _dp_class_ns_Wrapper = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_Wrapper = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         Wrapper = _dp_define_class_Wrapper(_dp_class_ns_Wrapper, globals())
#         return __dp_NONE

# class_with_base


class D(Base):
    pass


# ==

# function _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg):
#     function_id: 0
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "D")
#         return __dp_NONE

# function _dp_define_class_D(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("D", _dp_class_ns_fn, __dp_tuple(Base), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 2
#     block bb1:
#         _dp_class_ns_D = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_D = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         D = _dp_define_class_D(_dp_class_ns_D, globals())
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
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "outer.<locals>.Inner")
#         y = x
#         return __dp_NONE

# function outer.<locals>._dp_define_class_Inner(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Inner", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 6, ())

# function outer():
#     function_id: 2
#     block bb1:
#         x = "outer"
#         _dp_class_ns_Inner = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_Inner = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         Inner = _dp_define_class_Inner(_dp_class_ns_Inner, globals())
#         return Inner.y

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         outer = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
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
#     block bb1:
#         del __class__
#         __dp_call_super(super, __dp_cell_ref("__class__"), x)
#         return __dp_NONE

# function _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "X")
#         f = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# function _dp_define_class_X(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("X", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, True, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         _dp_class_ns_X = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_X = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         X = _dp_define_class_X(_dp_class_ns_X, globals())
#         return __dp_NONE

# nested classes


class A:
    class B:
        pass


# ==

# function A._dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
#     function_id: 0
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A.B")
#         return __dp_NONE

# function A._dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A")
#         _dp_class_ns_B = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_B = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         B = _dp_define_class_B(_dp_class_ns_B, _dp_class_ns)
#         return __dp_NONE

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 4
#     block bb1:
#         _dp_class_ns_A = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_A = __dp_make_function(3, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         A = _dp_define_class_A(_dp_class_ns_A, globals())
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
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "B")
#         return __dp_NONE

# function foo.<locals>.A._dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 7, ())

# function foo.<locals>._dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "foo.<locals>.A")
#         _dp_class_ns_B = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_B = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         B = _dp_define_class_B(_dp_class_ns_B, _dp_class_ns)
#         return __dp_NONE

# function foo.<locals>._dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function foo():
#     function_id: 4
#     block bb1:
#         _dp_class_ns_A = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_A = __dp_make_function(3, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         A = _dp_define_class_A(_dp_class_ns_A, globals())
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 5
#     block bb1:
#         foo = __dp_make_function(4, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE
