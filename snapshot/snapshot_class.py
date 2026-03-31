# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==

# function C.m(self):
#     function_id: 0
#     block bb1:
#         return GetAttr(self, "x")

# function C.__annotate_func__(_dp_format, _dp):
#     function_id: 1
#     block bb1:
#         if_term GetAttr(_dp, "eq")(_dp_format, 4):
#             then:
#                 block bb5:
#                     return GetAttr(_dp, "dict")(__dp_tuple(__dp_tuple("x", "int")))
#             else:
#                 block bb2:
#                     if_term GetAttr(_dp, "gt")(_dp_format, 2):
#                         then:
#                             block bb4:
#                                 raise GetAttr(GetAttr(_dp, "builtins"), "NotImplementedError")
#                         else:
#                             block bb3:
#                                 return GetAttr(_dp, "dict")(__dp_tuple(__dp_tuple("x", int)))

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         x = 1
#         m = MakeFunction(0, Function, __dp_tuple(), __dp_NONE)
#         __annotate_func__ = MakeFunction(1, Function, __dp_tuple(runtime), __dp_NONE)
#         return __dp_NONE

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 3, __dp_tuple())

# function _dp_module_init():
#     function_id: 4
#     block bb1:
#         _dp_class_ns_C = MakeFunction(2, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_C = MakeFunction(3, Function, __dp_tuple(__dp_NONE), __dp_NONE)
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
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "Wrapper")
#         open = MakeFunction(0, Function, __dp_tuple("r", "utf8"), __dp_NONE)
#         return __dp_NONE

# function _dp_define_class_Wrapper(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Wrapper", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 3, __dp_tuple())

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         _dp_class_ns_Wrapper = MakeFunction(1, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_Wrapper = MakeFunction(2, Function, __dp_tuple(__dp_NONE), __dp_NONE)
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
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "D")
#         return __dp_NONE

# function _dp_define_class_D(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("D", _dp_class_ns_fn, __dp_tuple(Base), _dp_prepare_dict, __dp_FALSE, 3, __dp_tuple())

# function _dp_module_init():
#     function_id: 2
#     block bb1:
#         _dp_class_ns_D = MakeFunction(0, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_D = MakeFunction(1, Function, __dp_tuple(__dp_NONE), __dp_NONE)
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
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "outer.<locals>.Inner")
#         y = x
#         return __dp_NONE

# function outer.<locals>._dp_define_class_Inner(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Inner", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 6, __dp_tuple())

# function outer():
#     function_id: 2
#     block bb1:
#         x = "outer"
#         _dp_class_ns_Inner = MakeFunction(0, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_Inner = MakeFunction(1, Function, __dp_tuple(__dp_NONE), __dp_NONE)
#         Inner = _dp_define_class_Inner(_dp_class_ns_Inner, globals())
#         return GetAttr(Inner, "y")

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         outer = MakeFunction(2, Function, __dp_tuple(), __dp_NONE)
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
#         __dp_call_super(super, CellRefForName("__class__"), x)
#         return __dp_NONE

# function _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "X")
#         f = MakeFunction(0, Function, __dp_tuple(), __dp_NONE)
#         return __dp_NONE

# function _dp_define_class_X(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("X", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_TRUE, 3, __dp_tuple())

# function _dp_module_init():
#     function_id: 3
#     block bb1:
#         _dp_class_ns_X = MakeFunction(1, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_X = MakeFunction(2, Function, __dp_tuple(__dp_NONE), __dp_NONE)
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
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A.B")
#         return __dp_NONE

# function A._dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 4, __dp_tuple())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A")
#         _dp_class_ns_B = MakeFunction(0, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_B = MakeFunction(1, Function, __dp_tuple(__dp_NONE), __dp_NONE)
#         B = _dp_define_class_B(_dp_class_ns_B, _dp_class_ns)
#         return __dp_NONE

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 3, __dp_tuple())

# function _dp_module_init():
#     function_id: 4
#     block bb1:
#         _dp_class_ns_A = MakeFunction(2, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_A = MakeFunction(3, Function, __dp_tuple(__dp_NONE), __dp_NONE)
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
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "B")
#         return __dp_NONE

# function foo.<locals>.A._dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 1
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 7, __dp_tuple())

# function foo.<locals>._dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
#     function_id: 2
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         SetItem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "foo.<locals>.A")
#         _dp_class_ns_B = MakeFunction(0, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_B = MakeFunction(1, Function, __dp_tuple(__dp_NONE), __dp_NONE)
#         B = _dp_define_class_B(_dp_class_ns_B, _dp_class_ns)
#         return __dp_NONE

# function foo.<locals>._dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 3
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, __dp_FALSE, 4, __dp_tuple())

# function foo():
#     function_id: 4
#     block bb1:
#         _dp_class_ns_A = MakeFunction(2, Function, __dp_tuple(), __dp_NONE)
#         _dp_define_class_A = MakeFunction(3, Function, __dp_tuple(__dp_NONE), __dp_NONE)
#         A = _dp_define_class_A(_dp_class_ns_A, globals())
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 5
#     block bb1:
#         foo = MakeFunction(4, Function, __dp_tuple(), __dp_NONE)
#         return __dp_NONE
