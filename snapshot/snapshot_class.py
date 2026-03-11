# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==

# module_init: _dp_module_init

# function m(self) [kind=function, bind=m, target=local, qualname=C.m]
#     block m_start:
#         return self.x

# function __annotate_func__(_dp_format, _dp = __dp__) [kind=function, bind=__annotate_func__, target=local, qualname=C.__annotate_func__]
#     block __annotate_func___start:
#         if_term _dp.eq(_dp_format, 4):
#             then:
#                 block _dp_bb___annotate_func___1_then:
#                     jump __annotate_func___3
#             else:
#                 block _dp_bb___annotate_func___1_else:
#                     jump __annotate_func___2
#         block __annotate_func___3:
#             return _dp.dict(__dp_tuple(("x", "int")))
#         block __annotate_func___2:
#             if_term _dp.gt(_dp_format, 2):
#                 then:
#                     block _dp_bb___annotate_func___2_then:
#                         jump __annotate_func___1
#                 else:
#                     block _dp_bb___annotate_func___2_else:
#                         jump __annotate_func___0
#             block __annotate_func___1:
#                 raise _dp.builtins.NotImplementedError
#             block __annotate_func___0:
#                 return _dp.dict(__dp_tuple(("x", int)))

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_C, target=local, qualname=_dp_class_ns_C]
#     block _dp_class_ns_C_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "x", 1)
#         def m(self): ...
#         def __annotate_func__(_dp_format, _dp = __dp__): ...
#         return

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_C, target=local, qualname=_dp_define_class_C]
#     block _dp_define_class_C_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))
#         return

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==

# module_init: _dp_module_init

# function open(self, mode: __dp_class_lookup_global(_dp_class_ns, "str", globals()) = "r", *, encoding: __dp_class_lookup_global(_dp_class_ns, "str", globals()) = "utf8") [kind=function, bind=open, target=local, qualname=Wrapper.open]
#     block open_start:
#         return open(mode, encoding=encoding)

# function _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_Wrapper, target=local, qualname=_dp_class_ns_Wrapper]
#     block _dp_class_ns_Wrapper_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "Wrapper")
#         def open(self, mode: __dp_class_lookup_global(_dp_class_ns, "str", globals()) = "r", *, encoding: __dp_class_lookup_global(_dp_class_ns, "str", globals()) = "utf8"): ...
#         return

# function _dp_define_class_Wrapper(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_Wrapper, target=local, qualname=_dp_define_class_Wrapper]
#     block _dp_define_class_Wrapper_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Wrapper", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_Wrapper(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         __dp_store_global(globals(), "Wrapper", _dp_define_class_Wrapper(_dp_class_ns_Wrapper, globals()))
#         return

# class_with_base


class D(Base):
    pass


# ==

# module_init: _dp_module_init

# function _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_D, target=local, qualname=_dp_class_ns_D]
#     block _dp_class_ns_D_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "D")
#         return

# function _dp_define_class_D(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_D, target=local, qualname=_dp_define_class_D]
#     block _dp_define_class_D_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("D", _dp_class_ns_fn, __dp_tuple(Base), _dp_prepare_dict, False, 3, ())

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_D(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
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

# function _dp_class_ns_Inner(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_Inner, target=local, qualname=outer.<locals>._dp_class_ns_Inner]
#     block _dp_class_ns_Inner_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "outer.<locals>.Inner")
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "y", __dp_class_lookup_cell(_dp_class_ns, "x", _dp_cell_x))
#         return

# function _dp_define_class_Inner(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_Inner, target=local, qualname=outer.<locals>._dp_define_class_Inner]
#     block _dp_define_class_Inner_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("Inner", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 6, ())

# function outer() [kind=function, bind=outer, target=module_global, qualname=outer]
#     block outer_start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, "outer")
#         def _dp_class_ns_Inner(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_Inner(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         Inner = _dp_define_class_Inner(_dp_class_ns_Inner, globals())
#         return Inner.y

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def outer(): ...
#         return

# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


# ==

# module_init: _dp_module_init

# function f(x) [kind=function, bind=f, target=local, qualname=X.f]
#     block f_start:
#         __dp_delattr(_dp_classcell, "cell_contents")
#         __dp_call_super(super, _dp_classcell, x)
#         return

# function _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_X, target=local, qualname=_dp_class_ns_X]
#     block _dp_class_ns_X_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "X")
#         def f(x): ...
#         return

# function _dp_define_class_X(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_X, target=local, qualname=_dp_define_class_X]
#     block _dp_define_class_X_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("X", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, True, 3, ())

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_X(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         __dp_store_global(globals(), "X", _dp_define_class_X(_dp_class_ns_X, globals()))
#         return

# nested classes


class A:
    class B:
        pass


# ==

# module_init: _dp_module_init

# function _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_B, target=local, qualname=A._dp_class_ns_B]
#     block _dp_class_ns_B_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A.B")
#         return

# function _dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_B, target=local, qualname=A._dp_define_class_B]
#     block _dp_define_class_B_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_A, target=local, qualname=_dp_class_ns_A]
#     block _dp_class_ns_A_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "A")
#         def _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "B", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))
#         return

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_A, target=local, qualname=_dp_define_class_A]
#     block _dp_define_class_A_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
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

# function _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_B, target=local, qualname=foo.<locals>.A._dp_class_ns_B]
#     block _dp_class_ns_B_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "B")
#         return

# function _dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_B, target=local, qualname=foo.<locals>.A._dp_define_class_B]
#     block _dp_define_class_B_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("B", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 7, ())

# function _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_A, target=local, qualname=foo.<locals>._dp_class_ns_A]
#     block _dp_class_ns_A_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "foo.<locals>.A")
#         def _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_B(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         __dp_store_global(globals(), "B", _dp_define_class_B(_dp_class_ns_B, _dp_class_ns))
#         return

# function _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_A, target=local, qualname=foo.<locals>._dp_define_class_A]
#     block _dp_define_class_A_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("A", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 4, ())

# function foo() [kind=function, bind=foo, target=module_global, qualname=foo]
#     block foo_start:
#         def _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg): ...
#         def _dp_define_class_A(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#         A = _dp_define_class_A(_dp_class_ns_A, globals())
#         return

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def foo(): ...
#         return
