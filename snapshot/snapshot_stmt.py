# import_simple

import a

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_store_global(globals(), "a", __dp_import_("a", __spec__))
#         return

# import_dotted_alias

import a.b as c

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_store_global(globals(), "c", __dp_import_attr(__dp_import_("a.b", __spec__), "b"))
#         return

# import_from_alias

from pkg.mod import name as alias

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_import_1 = __dp_import_("pkg.mod", __spec__, __dp_list(__dp_tuple("name")))
#         __dp_store_global(globals(), "alias", __dp_import_attr(_dp_import_1, "name"))
#         return

# decorator_function


@dec
def f():
    pass


# ==

# module_init: _dp_module_init

# function f() [kind=function, bind=f, target=module_global, qualname=f]
#     block f_start:
#         pass
#         return

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def f(): ...
#         return

# assign_attr

obj.x = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return

# assign_subscript

obj[i] = v

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)
#         return

# assign_tuple_unpack

a, b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#         __dp_store_global(globals(), "a", __dp_getitem(_dp_tmp_1, 0))
#         __dp_store_global(globals(), "b", __dp_getitem(_dp_tmp_1, 1))
#         _dp_tmp_1 = __dp_DELETED
#         return

# assign_star_unpack

a, *b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#         __dp_store_global(globals(), "a", __dp_getitem(_dp_tmp_1, 0))
#         __dp_store_global(globals(), "b", __dp_list(__dp_getitem(_dp_tmp_1, 1)))
#         _dp_tmp_1 = __dp_DELETED
#         return

# assign_multi_targets

a = b = f()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_tmp_1 = f()
#         __dp_store_global(globals(), "a", _dp_tmp_1)
#         __dp_store_global(globals(), "b", _dp_tmp_1)
#         return

# ann_assign_simple

x: int = 1

# ==

# module_init: _dp_module_init

# function __annotate__(_dp_format, _dp = __dp__) [kind=function, bind=__annotate__, target=module_global, qualname=__annotate__]
#     block __annotate___start:
#         if_term _dp.eq(_dp_format, 4):
#             then:
#                 block _dp_bb___annotate___1_then:
#                     jump __annotate___3
#             else:
#                 block _dp_bb___annotate___1_else:
#                     jump __annotate___2
#         block __annotate___3:
#             return _dp.dict(__dp_tuple(("x", "int")))
#         block __annotate___2:
#             if_term _dp.gt(_dp_format, 2):
#                 then:
#                     block _dp_bb___annotate___2_then:
#                         jump __annotate___1
#                 else:
#                     block _dp_bb___annotate___2_else:
#                         jump __annotate___0
#             block __annotate___1:
#                 raise _dp.builtins.NotImplementedError
#             block __annotate___0:
#                 return _dp.dict(__dp_tuple(("x", int)))

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_store_global(globals(), "x", 1)
#         def __annotate__(_dp_format, _dp = __dp__): ...
#         return

# ann_assign_attr

obj.x: int = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return

# aug_assign_attr

obj.x += 1

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))
#         return

# delete_mixed

del obj.x, obj[i], x

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         __dp_delattr(obj, "x")
#         __dp_delitem(obj, i)
#         __dp_delitem(globals(), "x")
#         return

# assert_no_msg

assert cond

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         if_term __debug__:
#             then:
#                 block _dp_bb__dp_module_init_1_then:
#                     jump _dp_module_init_1
#             else:
#                 block _dp_bb__dp_module_init_1_else:
#                     jump _dp_module_init_2
#         block _dp_module_init_1:
#             if_term __dp_not_(cond):
#                 then:
#                     block _dp_bb__dp_module_init_2_then:
#                         jump _dp_module_init_0
#                 else:
#                     block _dp_bb__dp_module_init_2_else:
#                         jump _dp_module_init_2
#             block _dp_module_init_0:
#                 raise __dp_AssertionError
#         block _dp_module_init_2:
#             return

# assert_with_msg

assert cond, "oops"

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         if_term __debug__:
#             then:
#                 block _dp_bb__dp_module_init_1_then:
#                     jump _dp_module_init_1
#             else:
#                 block _dp_bb__dp_module_init_1_else:
#                     jump _dp_module_init_2
#         block _dp_module_init_1:
#             if_term __dp_not_(cond):
#                 then:
#                     block _dp_bb__dp_module_init_2_then:
#                         jump _dp_module_init_0
#                 else:
#                     block _dp_bb__dp_module_init_2_else:
#                         jump _dp_module_init_2
#             block _dp_module_init_0:
#                 raise __dp_AssertionError("oops")
#         block _dp_module_init_2:
#             return

# raise_from

raise E from cause

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         raise __dp_raise_from(E, cause)

# try_except_typed

try:
    f()
except E as e:
    g(e)
except:
    h()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         try_jump:
#             body_label: _dp_module_init_6
#             except_label: _dp_module_init_5
#         block _dp_module_init_6:
#             f()
#             return
#         block _dp_module_init_5:
#             if_term __dp_exception_matches(__dp_current_exception(), E):
#                 then:
#                     block _dp_bb__dp_module_init_3_then:
#                         jump _dp_module_init_3
#                 else:
#                     block _dp_bb__dp_module_init_3_else:
#                         jump _dp_module_init_4
#             block _dp_module_init_3:
#                 __dp_store_global(globals(), "e", __dp_current_exception())
#                 try_jump:
#                     body_label: _dp_module_init_2
#                     except_label: _dp_module_init_1
#                 block _dp_module_init_2:
#                     g(__dp_load_global(globals(), "e"))
#                     jump _dp_module_init_0
#                     block _dp_module_init_0:
#                         __dp_delitem_quietly(globals(), "e")
#                         return
#                 block _dp_module_init_1:
#                     __dp_delitem_quietly(globals(), "e")
#                     raise
#             block _dp_module_init_4:
#                 h()
#                 return

# for_else

for x in it:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_iter_1 = __dp_iter(it)
#         jump _dp_module_init_3
#         block _dp_module_init_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_module_init_3_then:
#                         jump _dp_module_init_0
#                 else:
#                     block _dp_bb__dp_module_init_3_else:
#                         jump _dp_module_init_2
#             block _dp_module_init_0:
#                 done()
#                 return
#             block _dp_module_init_2:
#                 x = _dp_tmp_2
#                 _dp_tmp_2 = None
#                 jump _dp_module_init_1
#                 block _dp_module_init_1:
#                     __dp_store_global(globals(), "x", x)
#                     body()
#                     jump _dp_module_init_3

# while_else

while cond:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_0:
#         done()
#         return
#     block _dp_module_init_1:
#         body()
#         jump _dp_module_init_start
#     block _dp_module_init_start:
#         if_term cond:
#             then:
#                 block _dp_bb__dp_module_init_1_then:
#                     jump _dp_module_init_1
#             else:
#                 block _dp_bb__dp_module_init_1_else:
#                     jump _dp_module_init_0

# with_as

with cm as x:
    body()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         jump _dp_module_init_7
#         block _dp_module_init_7:
#             _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
#             x = __dp_contextmanager_enter(cm)
#             _dp_with_ok_2 = True
#             try_jump:
#                 body_label: _dp_module_init_6
#                 except_label: _dp_module_init_5
#             block _dp_module_init_6:
#                 body()
#                 jump _dp_module_init_4
#             block _dp_module_init_4:
#                 _dp_try_exc_3 = None
#                 jump _dp_module_init_3
#                 block _dp_module_init_3:
#                     _dp_try_exc_3 = __dp_current_exception()
#                     if_term _dp_with_ok_2:
#                         then:
#                             block _dp_bb__dp_module_init_5_then:
#                                 jump _dp_module_init_2
#                         else:
#                             block _dp_bb__dp_module_init_5_else:
#                                 jump _dp_module_init_1
#                     block _dp_module_init_2:
#                         __dp_contextmanager_exit(_dp_with_exit_1, None)
#                         jump _dp_module_init_1
#                     block _dp_module_init_1:
#                         _dp_with_exit_1 = None
#                         if_term __dp_is_not(_dp_try_exc_3, None):
#                             then:
#                                 block _dp_bb__dp_module_init_6_then:
#                                     jump _dp_module_init_0
#                             else:
#                                 block _dp_bb__dp_module_init_6_else:
#                                     jump _dp_module_init_8
#                         block _dp_module_init_0:
#                             raise _dp_try_exc_3
#                         block _dp_module_init_8:
#                             return
#             block _dp_module_init_5:
#                 _dp_with_ok_2 = False
#                 __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#                 jump _dp_module_init_4

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# module_init: _dp_module_init

# function inner() [kind=function, bind=inner, target=module_global, qualname=inner]
#     block inner_start:
#         value = 1
#         return value

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def inner(): ...
#         return

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=_dp_listcomp_3]
#     block _dp_listcomp_3_start:
#         _dp_tmp_1 = __dp_list(__dp_tuple())
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_listcomp_3_3
#         block _dp_listcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_3_then:
#                         jump _dp_listcomp_3_0
#                 else:
#                     block _dp_bb__dp_listcomp_3_3_else:
#                         jump _dp_listcomp_3_2
#             block _dp_listcomp_3_0:
#                 return _dp_tmp_1
#             block _dp_listcomp_3_2:
#                 x = _dp_tmp_2
#                 _dp_tmp_2 = None
#                 jump _dp_listcomp_3_1
#                 block _dp_listcomp_3_1:
#                     _dp_tmp_1.append(x)
#                     jump _dp_listcomp_3_3

# function _dp_setcomp_6(_dp_iter_5) [kind=function, bind=_dp_setcomp_6, target=local, qualname=_dp_setcomp_6]
#     block _dp_setcomp_6_start:
#         _dp_tmp_4 = set()
#         _dp_iter_1 = __dp_iter(_dp_iter_5)
#         jump _dp_setcomp_6_3
#         block _dp_setcomp_6_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_setcomp_6_3_then:
#                         jump _dp_setcomp_6_0
#                 else:
#                     block _dp_bb__dp_setcomp_6_3_else:
#                         jump _dp_setcomp_6_2
#             block _dp_setcomp_6_0:
#                 return _dp_tmp_4
#             block _dp_setcomp_6_2:
#                 x = _dp_tmp_2
#                 _dp_tmp_2 = None
#                 jump _dp_setcomp_6_1
#                 block _dp_setcomp_6_1:
#                     _dp_tmp_4.add(x)
#                     jump _dp_setcomp_6_3

# function _dp_dictcomp_9(_dp_iter_8) [kind=function, bind=_dp_dictcomp_9, target=local, qualname=_dp_dictcomp_9]
#     block _dp_dictcomp_9_start:
#         _dp_tmp_7 = __dp_dict()
#         _dp_iter_1 = __dp_iter(_dp_iter_8)
#         jump _dp_dictcomp_9_3
#         block _dp_dictcomp_9_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_dictcomp_9_3_then:
#                         jump _dp_dictcomp_9_0
#                 else:
#                     block _dp_bb__dp_dictcomp_9_3_else:
#                         jump _dp_dictcomp_9_2
#             block _dp_dictcomp_9_0:
#                 return _dp_tmp_7
#             block _dp_dictcomp_9_2:
#                 _dp_tmp_4 = __dp_unpack(_dp_tmp_2, __dp_tuple(True, True))
#                 k = __dp_getitem(_dp_tmp_4, 0)
#                 v = __dp_getitem(_dp_tmp_4, 1)
#                 del _dp_tmp_4
#                 _dp_tmp_2 = None
#                 jump _dp_dictcomp_9_1
#                 block _dp_dictcomp_9_1:
#                     __dp_setitem(_dp_tmp_7, k, v)
#                     jump _dp_dictcomp_9_3

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def _dp_listcomp_3(_dp_iter_2): ...
#         __dp_store_global(globals(), "xs", _dp_listcomp_3(it))
#         def _dp_setcomp_6(_dp_iter_5): ...
#         __dp_store_global(globals(), "ys", _dp_setcomp_6(it))
#         def _dp_dictcomp_9(_dp_iter_8): ...
#         __dp_store_global(globals(), "zs", _dp_dictcomp_9(items))
#         return

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=f.<locals>._dp_listcomp_3]
#     block _dp_listcomp_3_start:
#         _dp_tmp_1 = __dp_list(__dp_tuple())
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_listcomp_3_4
#         block _dp_listcomp_3_4:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_3_then:
#                         jump _dp_listcomp_3_0
#                 else:
#                     block _dp_bb__dp_listcomp_3_3_else:
#                         jump _dp_listcomp_3_3
#             block _dp_listcomp_3_0:
#                 return _dp_tmp_1
#             block _dp_listcomp_3_3:
#                 x = _dp_tmp_2
#                 _dp_tmp_2 = None
#                 jump _dp_listcomp_3_2
#                 block _dp_listcomp_3_2:
#                     if_term __dp_gt(x, 0):
#                         then:
#                             block _dp_bb__dp_listcomp_3_7_then:
#                                 jump _dp_listcomp_3_1
#                         else:
#                             block _dp_bb__dp_listcomp_3_7_else:
#                                 jump _dp_listcomp_3_4
#                     block _dp_listcomp_3_1:
#                         _dp_tmp_1.append(x)
#                         jump _dp_listcomp_3_4

# function f() [kind=function, bind=f, target=module_global, qualname=f]
#     block f_start:
#         def _dp_listcomp_3(_dp_iter_2): ...
#         return _dp_listcomp_3(it)

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def f(): ...
#         return

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=C._dp_listcomp_3]
#     block _dp_listcomp_3_start:
#         _dp_tmp_1 = __dp_list(__dp_tuple())
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_listcomp_3_3
#         block _dp_listcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_3_then:
#                         jump _dp_listcomp_3_0
#                 else:
#                     block _dp_bb__dp_listcomp_3_3_else:
#                         jump _dp_listcomp_3_2
#             block _dp_listcomp_3_0:
#                 return _dp_tmp_1
#             block _dp_listcomp_3_2:
#                 x = _dp_tmp_2
#                 _dp_tmp_2 = None
#                 jump _dp_listcomp_3_1
#                 block _dp_listcomp_3_1:
#                     _dp_tmp_1.append(x)
#                     jump _dp_listcomp_3_3

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_C, target=local, qualname=_dp_class_ns_C]
#     block _dp_class_ns_C_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         def _dp_listcomp_3(_dp_iter_2): ...
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "xs", _dp_listcomp_3(__dp_class_lookup_global(_dp_class_ns, "it", globals())))
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

# with_multi

with a as x, b as y:
    body()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         jump _dp_module_init_14
#         block _dp_module_init_14:
#             _dp_with_exit_4 = __dp_contextmanager_get_exit(a)
#             x = __dp_contextmanager_enter(a)
#             _dp_with_ok_5 = True
#             try_jump:
#                 body_label: _dp_module_init_13
#                 except_label: _dp_module_init_5
#             block _dp_module_init_13:
#                 _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
#                 y = __dp_contextmanager_enter(b)
#                 _dp_with_ok_2 = True
#                 try_jump:
#                     body_label: _dp_module_init_12
#                     except_label: _dp_module_init_11
#                 block _dp_module_init_12:
#                     body()
#                     jump _dp_module_init_10
#                 block _dp_module_init_10:
#                     _dp_try_exc_11 = None
#                     jump _dp_module_init_9
#                     block _dp_module_init_9:
#                         _dp_try_exc_11 = __dp_current_exception()
#                         if_term _dp_with_ok_2:
#                             then:
#                                 block _dp_bb__dp_module_init_13_then:
#                                     jump _dp_module_init_8
#                             else:
#                                 block _dp_bb__dp_module_init_13_else:
#                                     jump _dp_module_init_7
#                         block _dp_module_init_8:
#                             __dp_contextmanager_exit(_dp_with_exit_1, None)
#                             jump _dp_module_init_7
#                         block _dp_module_init_7:
#                             _dp_with_exit_1 = None
#                             if_term __dp_is_not(_dp_try_exc_11, None):
#                                 then:
#                                     block _dp_bb__dp_module_init_14_then:
#                                         jump _dp_module_init_6
#                                 else:
#                                     block _dp_bb__dp_module_init_14_else:
#                                         jump _dp_module_init_4
#                             block _dp_module_init_6:
#                                 raise _dp_try_exc_11
#                 block _dp_module_init_11:
#                     _dp_with_ok_2 = False
#                     __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#                     jump _dp_module_init_10
#             block _dp_module_init_4:
#                 _dp_try_exc_3 = None
#                 jump _dp_module_init_3
#                 block _dp_module_init_3:
#                     _dp_try_exc_3 = __dp_current_exception()
#                     if_term _dp_with_ok_5:
#                         then:
#                             block _dp_bb__dp_module_init_5_then:
#                                 jump _dp_module_init_2
#                         else:
#                             block _dp_bb__dp_module_init_5_else:
#                                 jump _dp_module_init_1
#                     block _dp_module_init_2:
#                         __dp_contextmanager_exit(_dp_with_exit_4, None)
#                         jump _dp_module_init_1
#                     block _dp_module_init_1:
#                         _dp_with_exit_4 = None
#                         if_term __dp_is_not(_dp_try_exc_3, None):
#                             then:
#                                 block _dp_bb__dp_module_init_6_then:
#                                     jump _dp_module_init_0
#                             else:
#                                 block _dp_bb__dp_module_init_6_else:
#                                     jump _dp_module_init_15
#                         block _dp_module_init_0:
#                             raise _dp_try_exc_3
#                         block _dp_module_init_15:
#                             return
#             block _dp_module_init_5:
#                 _dp_with_ok_5 = False
#                 __dp_contextmanager_exit(_dp_with_exit_4, __dp_exc_info())
#                 jump _dp_module_init_4

# async_for


async def run():
    async for x in ait:
        body()


# ==

# module_init: _dp_module_init

# function run() [kind=generator, bind=run, target=module_global, qualname=run]
#     block run_dispatch:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block run_dispatch_then:
#                     jump run_dispatch_send_table
#             else:
#                 block run_dispatch_else:
#                     jump run_dispatch_throw_table
#         block run_dispatch_send_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_done, run_dispatch_send_target_1, run_dispatch_send_target_2, run_dispatch_send_target_3] default run_invalid
#             block run_done:
#                 return
#             block run_dispatch_send_target_1:
#                 jump run_start
#                 block run_start:
#                     jump run_24
#             block run_dispatch_send_target_2:
#                 jump run_7
#             block run_dispatch_send_target_3:
#                 jump run_26
#         block run_24:
#             _dp_iter_2 = __dp_aiter(ait)
#             jump run_19
#         block run_19:
#             jump run_0
#             block run_0:
#                 _dp_yield_from_iter_7 = iter(__dp_await_iter(__dp_anext_or_sentinel(_dp_iter_2)))
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_7)
#                 try_jump:
#                     body_label: run_1
#                     except_label: run_2
#         block run_1:
#             _dp_yield_from_y_8 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             jump run_6
#         block run_6:
#             return _dp_yield_from_y_8
#         block run_2:
#             _dp_try_exc_11 = __dp_current_exception()
#             if_term __dp_exception_matches(_dp_try_exc_11, StopIteration):
#                 then:
#                     block _dp_bb_run_18_then:
#                         jump run_3
#                 else:
#                     block _dp_bb_run_18_else:
#                         jump run_5
#             block run_3:
#                 _dp_yield_from_result_10 = _dp_try_exc_11.value
#                 jump run_4
#                 block run_4:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     jump run_20
#                     block run_20:
#                         _dp_tmp_3 = _dp_yield_from_result_10
#                         jump run_23
#                         block run_23:
#                             if_term __dp_is_(_dp_tmp_3, __dp__.ITER_COMPLETE):
#                                 then:
#                                     block _dp_bb_run_4_then:
#                                         jump run_27
#                                 else:
#                                     block _dp_bb_run_4_else:
#                                         jump run_22
#                             block run_27:
#                                 return
#                             block run_22:
#                                 x = _dp_tmp_3
#                                 _dp_tmp_3 = None
#                                 jump run_21
#                                 block run_21:
#                                     body()
#                                     jump run_19
#             block run_5:
#                 _dp_yield_from_raise_13 = _dp_try_exc_11
#                 jump run_12
#         block run_12:
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_13
#         block run_7:
#             _dp_yield_from_sent_9 = _dp_send_value
#             _dp_yield_from_exc_12 = _dp_resume_exc
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_12, None):
#                 then:
#                     block _dp_bb_run_23_then:
#                         jump run_8
#                 else:
#                     block _dp_bb_run_23_else:
#                         jump run_16
#             block run_8:
#                 if_term __dp_exception_matches(_dp_yield_from_exc_12, GeneratorExit):
#                     then:
#                         block _dp_bb_run_24_then:
#                             jump run_9
#                     else:
#                         block _dp_bb_run_24_else:
#                             jump run_13
#                 block run_9:
#                     _dp_yield_from_close_14 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                     if_term __dp_is_not(_dp_yield_from_close_14, None):
#                         then:
#                             block _dp_bb_run_25_true:
#                                 jump run_10
#                         else:
#                             block _dp_bb_run_25_false:
#                                 jump run_11
#                     block run_10:
#                         _dp_yield_from_close_14()
#                         jump run_11
#                 block run_11:
#                     _dp_yield_from_raise_13 = _dp_yield_from_exc_12
#                     jump run_12
#                 block run_13:
#                     _dp_yield_from_throw_15 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                     if_term __dp_is_(_dp_yield_from_throw_15, None):
#                         then:
#                             block _dp_bb_run_28_true:
#                                 jump run_11
#                         else:
#                             block _dp_bb_run_28_false:
#                                 jump run_14
#                     block run_14:
#                         try_jump:
#                             body_label: run_15
#                             except_label: run_2
#                         block run_15:
#                             _dp_yield_from_y_8 = _dp_yield_from_throw_15(_dp_yield_from_exc_12)
#                             jump run_6
#             block run_16:
#                 try_jump:
#                     body_label: run_17
#                     except_label: run_2
#                 block run_17:
#                     if_term __dp_is_(_dp_yield_from_sent_9, None):
#                         then:
#                             block _dp_bb_run_32_then:
#                                 jump run_1
#                         else:
#                             block _dp_bb_run_32_else:
#                                 jump run_18
#                     block run_18:
#                         _dp_yield_from_y_8 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_9)
#                         jump run_6
#         block run_26:
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_run_40_true:
#                         jump run_25
#                 else:
#                     block _dp_bb_run_40_false:
#                         jump run_24
#             block run_25:
#                 raise _dp_resume_exc
#         block run_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#         block run_dispatch_throw_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_dispatch_throw_done, run_dispatch_throw_target_1, run_dispatch_throw_target_2, run_dispatch_throw_target_3] default run_invalid
#             block run_dispatch_throw_done:
#                 raise _dp_resume_exc
#             block run_dispatch_throw_target_1:
#                 jump run_dispatch_throw_unstarted
#                 block run_dispatch_throw_unstarted:
#                     raise _dp_resume_exc
#             block run_dispatch_throw_target_2:
#                 jump run_7
#             block run_dispatch_throw_target_3:
#                 jump run_26
#     block run_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 block run_uncaught_then:
#                     jump run_uncaught_set_done
#             else:
#                 block run_uncaught_else:
#                     jump run_uncaught_raise
#     block run_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_41)
#         jump run_uncaught_raise
#     block run_uncaught_raise:
#         raise _dp_uncaught_exc_41

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def run(): ...
#         return

# async_with


async def run():
    async with cm as x:
        body()


# ==

# module_init: _dp_module_init

# function run() [kind=generator, bind=run, target=module_global, qualname=run]
#     block run_dispatch:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block run_dispatch_then:
#                     jump run_dispatch_send_table
#             else:
#                 block run_dispatch_else:
#                     jump run_dispatch_throw_table
#         block run_dispatch_send_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_done, run_dispatch_send_target_1, run_dispatch_send_target_2, run_dispatch_send_target_3, run_dispatch_send_target_4, run_dispatch_send_target_5] default run_invalid
#             block run_done:
#                 return
#             block run_dispatch_send_target_1:
#                 jump run_start
#                 block run_start:
#                     jump run_69
#             block run_dispatch_send_target_2:
#                 jump run_9
#             block run_dispatch_send_target_3:
#                 jump run_33
#             block run_dispatch_send_target_4:
#                 jump run_56
#             block run_dispatch_send_target_5:
#                 jump run_71
#         block run_69:
#             _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
#             jump run_49
#             block run_49:
#                 _dp_yield_from_iter_71 = iter(__dp_await_iter(__dp_asynccontextmanager_aenter(cm)))
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_71)
#                 try_jump:
#                     body_label: run_50
#                     except_label: run_51
#         block run_50:
#             _dp_yield_from_y_72 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             jump run_55
#         block run_55:
#             return _dp_yield_from_y_72
#         block run_51:
#             _dp_try_exc_75 = __dp_current_exception()
#             if_term __dp_exception_matches(_dp_try_exc_75, StopIteration):
#                 then:
#                     block _dp_bb_run_82_then:
#                         jump run_52
#                 else:
#                     block _dp_bb_run_82_else:
#                         jump run_54
#             block run_52:
#                 _dp_yield_from_result_74 = _dp_try_exc_75.value
#                 jump run_53
#                 block run_53:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     jump run_68
#                     block run_68:
#                         x = _dp_yield_from_result_74
#                         jump run_48
#                         block run_48:
#                             _dp_with_ok_2 = True
#                             try_jump:
#                                 body_label: run_47
#                                 except_label: run_46
#                             block run_47:
#                                 body()
#                                 jump run_23
#                             block run_46:
#                                 _dp_with_ok_2 = False
#                                 jump run_26
#                                 block run_26:
#                                     _dp_yield_from_iter_40 = iter(__dp_await_iter(__dp_asynccontextmanager_aexit(_dp_with_exit_1, __dp_exc_info())))
#                                     __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_40)
#                                     try_jump:
#                                         body_label: run_27
#                                         except_label: run_28
#             block run_54:
#                 _dp_yield_from_raise_77 = _dp_try_exc_75
#                 jump run_61
#         block run_23:
#             _dp_try_exc_3 = None
#             jump run_22
#             block run_22:
#                 _dp_try_exc_3 = __dp_current_exception()
#                 if_term _dp_with_ok_2:
#                     then:
#                         block _dp_bb_run_5_then:
#                             jump run_21
#                     else:
#                         block _dp_bb_run_5_else:
#                             jump run_1
#                 block run_21:
#                     jump run_2
#                     block run_2:
#                         _dp_yield_from_iter_9 = iter(__dp_await_iter(__dp_asynccontextmanager_aexit(_dp_with_exit_1, None)))
#                         __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_9)
#                         try_jump:
#                             body_label: run_3
#                             except_label: run_4
#         block run_3:
#             _dp_yield_from_y_10 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             jump run_8
#         block run_8:
#             return _dp_yield_from_y_10
#         block run_4:
#             _dp_try_exc_12 = __dp_current_exception()
#             if_term __dp_exception_matches(_dp_try_exc_12, StopIteration):
#                 then:
#                     block _dp_bb_run_19_then:
#                         jump run_5
#                 else:
#                     block _dp_bb_run_19_else:
#                         jump run_7
#             block run_5:
#                 jump run_6
#                 block run_6:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     jump run_1
#             block run_7:
#                 _dp_yield_from_raise_14 = _dp_try_exc_12
#                 jump run_14
#         block run_1:
#             _dp_with_exit_1 = None
#             if_term __dp_is_not(_dp_try_exc_3, None):
#                 then:
#                     block _dp_bb_run_6_then:
#                         jump run_0
#                 else:
#                     block _dp_bb_run_6_else:
#                         jump run_72
#             block run_0:
#                 raise _dp_try_exc_3
#             block run_72:
#                 return
#         block run_14:
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_14
#         block run_27:
#             _dp_yield_from_y_41 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             jump run_32
#         block run_32:
#             return _dp_yield_from_y_41
#         block run_28:
#             _dp_try_exc_44 = __dp_current_exception()
#             if_term __dp_exception_matches(_dp_try_exc_44, StopIteration):
#                 then:
#                     block _dp_bb_run_51_then:
#                         jump run_29
#                 else:
#                     block _dp_bb_run_51_else:
#                         jump run_31
#             block run_29:
#                 _dp_yield_from_result_43 = _dp_try_exc_44.value
#                 jump run_30
#                 block run_30:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     jump run_45
#                     block run_45:
#                         _dp_with_suppress_3 = _dp_yield_from_result_43
#                         jump run_25
#                         block run_25:
#                             if_term not _dp_with_suppress_3:
#                                 then:
#                                     block _dp_bb_run_36_then:
#                                         jump run_24
#                                 else:
#                                     block _dp_bb_run_36_else:
#                                         jump run_23
#                             block run_24:
#                                 raise
#             block run_31:
#                 _dp_yield_from_raise_46 = _dp_try_exc_44
#                 jump run_38
#         block run_38:
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_46
#         block run_61:
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_77
#         block run_9:
#             _dp_yield_from_sent_11 = _dp_send_value
#             _dp_yield_from_exc_13 = _dp_resume_exc
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_13, None):
#                 then:
#                     block _dp_bb_run_24_then:
#                         jump run_10
#                 else:
#                     block _dp_bb_run_24_else:
#                         jump run_18
#             block run_10:
#                 if_term __dp_exception_matches(_dp_yield_from_exc_13, GeneratorExit):
#                     then:
#                         block _dp_bb_run_25_then:
#                             jump run_11
#                     else:
#                         block _dp_bb_run_25_else:
#                             jump run_15
#                 block run_11:
#                     _dp_yield_from_close_15 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                     if_term __dp_is_not(_dp_yield_from_close_15, None):
#                         then:
#                             block _dp_bb_run_26_true:
#                                 jump run_12
#                         else:
#                             block _dp_bb_run_26_false:
#                                 jump run_13
#                     block run_12:
#                         _dp_yield_from_close_15()
#                         jump run_13
#                 block run_13:
#                     _dp_yield_from_raise_14 = _dp_yield_from_exc_13
#                     jump run_14
#                 block run_15:
#                     _dp_yield_from_throw_16 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                     if_term __dp_is_(_dp_yield_from_throw_16, None):
#                         then:
#                             block _dp_bb_run_29_true:
#                                 jump run_13
#                         else:
#                             block _dp_bb_run_29_false:
#                                 jump run_16
#                     block run_16:
#                         try_jump:
#                             body_label: run_17
#                             except_label: run_4
#                         block run_17:
#                             _dp_yield_from_y_10 = _dp_yield_from_throw_16(_dp_yield_from_exc_13)
#                             jump run_8
#             block run_18:
#                 try_jump:
#                     body_label: run_19
#                     except_label: run_4
#                 block run_19:
#                     if_term __dp_is_(_dp_yield_from_sent_11, None):
#                         then:
#                             block _dp_bb_run_33_then:
#                                 jump run_3
#                         else:
#                             block _dp_bb_run_33_else:
#                                 jump run_20
#                     block run_20:
#                         _dp_yield_from_y_10 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_11)
#                         jump run_8
#         block run_33:
#             _dp_yield_from_sent_42 = _dp_send_value
#             _dp_yield_from_exc_45 = _dp_resume_exc
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_45, None):
#                 then:
#                     block _dp_bb_run_56_then:
#                         jump run_34
#                 else:
#                     block _dp_bb_run_56_else:
#                         jump run_42
#             block run_34:
#                 if_term __dp_exception_matches(_dp_yield_from_exc_45, GeneratorExit):
#                     then:
#                         block _dp_bb_run_57_then:
#                             jump run_35
#                     else:
#                         block _dp_bb_run_57_else:
#                             jump run_39
#                 block run_35:
#                     _dp_yield_from_close_47 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                     if_term __dp_is_not(_dp_yield_from_close_47, None):
#                         then:
#                             block _dp_bb_run_58_true:
#                                 jump run_36
#                         else:
#                             block _dp_bb_run_58_false:
#                                 jump run_37
#                     block run_36:
#                         _dp_yield_from_close_47()
#                         jump run_37
#                 block run_37:
#                     _dp_yield_from_raise_46 = _dp_yield_from_exc_45
#                     jump run_38
#                 block run_39:
#                     _dp_yield_from_throw_48 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                     if_term __dp_is_(_dp_yield_from_throw_48, None):
#                         then:
#                             block _dp_bb_run_61_true:
#                                 jump run_37
#                         else:
#                             block _dp_bb_run_61_false:
#                                 jump run_40
#                     block run_40:
#                         try_jump:
#                             body_label: run_41
#                             except_label: run_28
#                         block run_41:
#                             _dp_yield_from_y_41 = _dp_yield_from_throw_48(_dp_yield_from_exc_45)
#                             jump run_32
#             block run_42:
#                 try_jump:
#                     body_label: run_43
#                     except_label: run_28
#                 block run_43:
#                     if_term __dp_is_(_dp_yield_from_sent_42, None):
#                         then:
#                             block _dp_bb_run_65_then:
#                                 jump run_27
#                         else:
#                             block _dp_bb_run_65_else:
#                                 jump run_44
#                     block run_44:
#                         _dp_yield_from_y_41 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_42)
#                         jump run_32
#         block run_56:
#             _dp_yield_from_sent_73 = _dp_send_value
#             _dp_yield_from_exc_76 = _dp_resume_exc
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_76, None):
#                 then:
#                     block _dp_bb_run_87_then:
#                         jump run_57
#                 else:
#                     block _dp_bb_run_87_else:
#                         jump run_65
#             block run_57:
#                 if_term __dp_exception_matches(_dp_yield_from_exc_76, GeneratorExit):
#                     then:
#                         block _dp_bb_run_88_then:
#                             jump run_58
#                     else:
#                         block _dp_bb_run_88_else:
#                             jump run_62
#                 block run_58:
#                     _dp_yield_from_close_78 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                     if_term __dp_is_not(_dp_yield_from_close_78, None):
#                         then:
#                             block _dp_bb_run_89_true:
#                                 jump run_59
#                         else:
#                             block _dp_bb_run_89_false:
#                                 jump run_60
#                     block run_59:
#                         _dp_yield_from_close_78()
#                         jump run_60
#                 block run_60:
#                     _dp_yield_from_raise_77 = _dp_yield_from_exc_76
#                     jump run_61
#                 block run_62:
#                     _dp_yield_from_throw_79 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                     if_term __dp_is_(_dp_yield_from_throw_79, None):
#                         then:
#                             block _dp_bb_run_92_true:
#                                 jump run_60
#                         else:
#                             block _dp_bb_run_92_false:
#                                 jump run_63
#                     block run_63:
#                         try_jump:
#                             body_label: run_64
#                             except_label: run_51
#                         block run_64:
#                             _dp_yield_from_y_72 = _dp_yield_from_throw_79(_dp_yield_from_exc_76)
#                             jump run_55
#             block run_65:
#                 try_jump:
#                     body_label: run_66
#                     except_label: run_51
#                 block run_66:
#                     if_term __dp_is_(_dp_yield_from_sent_73, None):
#                         then:
#                             block _dp_bb_run_96_then:
#                                 jump run_50
#                         else:
#                             block _dp_bb_run_96_else:
#                                 jump run_67
#                     block run_67:
#                         _dp_yield_from_y_72 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_73)
#                         jump run_55
#         block run_71:
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_run_101_true:
#                         jump run_70
#                 else:
#                     block _dp_bb_run_101_false:
#                         jump run_69
#             block run_70:
#                 raise _dp_resume_exc
#         block run_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#         block run_dispatch_throw_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_dispatch_throw_done, run_dispatch_throw_target_1, run_dispatch_throw_target_2, run_dispatch_throw_target_3, run_dispatch_throw_target_4, run_dispatch_throw_target_5] default run_invalid
#             block run_dispatch_throw_done:
#                 raise _dp_resume_exc
#             block run_dispatch_throw_target_1:
#                 jump run_dispatch_throw_unstarted
#                 block run_dispatch_throw_unstarted:
#                     raise _dp_resume_exc
#             block run_dispatch_throw_target_2:
#                 jump run_9
#             block run_dispatch_throw_target_3:
#                 jump run_33
#             block run_dispatch_throw_target_4:
#                 jump run_56
#             block run_dispatch_throw_target_5:
#                 jump run_71
#     block run_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 block run_uncaught_then:
#                     jump run_uncaught_set_done
#             else:
#                 block run_uncaught_else:
#                     jump run_uncaught_raise
#     block run_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_102)
#         jump run_uncaught_raise
#     block run_uncaught_raise:
#         raise _dp_uncaught_exc_102

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def run(): ...
#         return

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_match_1 = value
#         if_term __dp_eq(_dp_match_1, 1):
#             then:
#                 block _dp_bb__dp_module_init_1_then:
#                     jump _dp_module_init_0
#             else:
#                 block _dp_bb__dp_module_init_1_else:
#                     jump _dp_module_init_1
#         block _dp_module_init_0:
#             one()
#             return
#         block _dp_module_init_1:
#             other()
#             return

# generator_yield


def gen():
    yield 1


# ==

# module_init: _dp_module_init

# function gen() [kind=generator, bind=gen, target=module_global, qualname=gen]
#     block gen_dispatch:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block gen_dispatch_then:
#                     jump gen_dispatch_send_table
#             else:
#                 block gen_dispatch_else:
#                     jump gen_dispatch_throw_table
#         block gen_dispatch_send_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_done, gen_dispatch_send_target_1, gen_dispatch_send_target_2] default gen_invalid
#             block gen_done:
#                 return
#             block gen_dispatch_send_target_1:
#                 jump gen_start
#                 block gen_start:
#                     return 1
#             block gen_dispatch_send_target_2:
#                 jump gen_1
#         block gen_1:
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_gen_3_true:
#                         jump gen_0
#                 else:
#                     block _dp_bb_gen_3_false:
#                         jump gen_2
#             block gen_0:
#                 raise _dp_resume_exc
#             block gen_2:
#                 return
#         block gen_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#         block gen_dispatch_throw_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_dispatch_throw_done, gen_dispatch_throw_target_1, gen_dispatch_throw_target_2] default gen_invalid
#             block gen_dispatch_throw_done:
#                 raise _dp_resume_exc
#             block gen_dispatch_throw_target_1:
#                 jump gen_dispatch_throw_unstarted
#                 block gen_dispatch_throw_unstarted:
#                     raise _dp_resume_exc
#             block gen_dispatch_throw_target_2:
#                 jump gen_1
#     block gen_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 block gen_uncaught_then:
#                     jump gen_uncaught_set_done
#             else:
#                 block gen_uncaught_else:
#                     jump gen_uncaught_raise
#     block gen_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_4)
#         jump gen_uncaught_raise
#     block gen_uncaught_raise:
#         raise _dp_uncaught_exc_4

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def gen(): ...
#         return

# yield_from


def gen():
    yield from it


# ==

# module_init: _dp_module_init

# function gen() [kind=generator, bind=gen, target=module_global, qualname=gen]
#     block gen_dispatch:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block gen_dispatch_then:
#                     jump gen_dispatch_send_table
#             else:
#                 block gen_dispatch_else:
#                     jump gen_dispatch_throw_table
#         block gen_dispatch_send_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_done, gen_dispatch_send_target_1, gen_dispatch_send_target_2] default gen_invalid
#             block gen_done:
#                 return
#             block gen_dispatch_send_target_1:
#                 jump gen_start
#                 block gen_start:
#                     jump gen_0
#                     block gen_0:
#                         _dp_yield_from_iter_2 = iter(it)
#                         __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_2)
#                         try_jump:
#                             body_label: gen_1
#                             except_label: gen_2
#             block gen_dispatch_send_target_2:
#                 jump gen_7
#         block gen_1:
#             _dp_yield_from_y_3 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             jump gen_6
#         block gen_6:
#             return _dp_yield_from_y_3
#         block gen_2:
#             _dp_try_exc_5 = __dp_current_exception()
#             if_term __dp_exception_matches(_dp_try_exc_5, StopIteration):
#                 then:
#                     block _dp_bb_gen_12_then:
#                         jump gen_3
#                 else:
#                     block _dp_bb_gen_12_else:
#                         jump gen_5
#             block gen_3:
#                 jump gen_4
#                 block gen_4:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     return
#             block gen_5:
#                 _dp_yield_from_raise_7 = _dp_try_exc_5
#                 jump gen_12
#         block gen_12:
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_7
#         block gen_7:
#             _dp_yield_from_sent_4 = _dp_send_value
#             _dp_yield_from_exc_6 = _dp_resume_exc
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_6, None):
#                 then:
#                     block _dp_bb_gen_17_then:
#                         jump gen_8
#                 else:
#                     block _dp_bb_gen_17_else:
#                         jump gen_16
#             block gen_8:
#                 if_term __dp_exception_matches(_dp_yield_from_exc_6, GeneratorExit):
#                     then:
#                         block _dp_bb_gen_18_then:
#                             jump gen_9
#                     else:
#                         block _dp_bb_gen_18_else:
#                             jump gen_13
#                 block gen_9:
#                     _dp_yield_from_close_8 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                     if_term __dp_is_not(_dp_yield_from_close_8, None):
#                         then:
#                             block _dp_bb_gen_19_true:
#                                 jump gen_10
#                         else:
#                             block _dp_bb_gen_19_false:
#                                 jump gen_11
#                     block gen_10:
#                         _dp_yield_from_close_8()
#                         jump gen_11
#                 block gen_11:
#                     _dp_yield_from_raise_7 = _dp_yield_from_exc_6
#                     jump gen_12
#                 block gen_13:
#                     _dp_yield_from_throw_9 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                     if_term __dp_is_(_dp_yield_from_throw_9, None):
#                         then:
#                             block _dp_bb_gen_22_true:
#                                 jump gen_11
#                         else:
#                             block _dp_bb_gen_22_false:
#                                 jump gen_14
#                     block gen_14:
#                         try_jump:
#                             body_label: gen_15
#                             except_label: gen_2
#                         block gen_15:
#                             _dp_yield_from_y_3 = _dp_yield_from_throw_9(_dp_yield_from_exc_6)
#                             jump gen_6
#             block gen_16:
#                 try_jump:
#                     body_label: gen_17
#                     except_label: gen_2
#                 block gen_17:
#                     if_term __dp_is_(_dp_yield_from_sent_4, None):
#                         then:
#                             block _dp_bb_gen_26_then:
#                                 jump gen_1
#                         else:
#                             block _dp_bb_gen_26_else:
#                                 jump gen_18
#                     block gen_18:
#                         _dp_yield_from_y_3 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_4)
#                         jump gen_6
#         block gen_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#         block gen_dispatch_throw_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_dispatch_throw_done, gen_dispatch_throw_target_1, gen_dispatch_throw_target_2] default gen_invalid
#             block gen_dispatch_throw_done:
#                 raise _dp_resume_exc
#             block gen_dispatch_throw_target_1:
#                 jump gen_dispatch_throw_unstarted
#                 block gen_dispatch_throw_unstarted:
#                     raise _dp_resume_exc
#             block gen_dispatch_throw_target_2:
#                 jump gen_7
#     block gen_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 block gen_uncaught_then:
#                     jump gen_uncaught_set_done
#             else:
#                 block gen_uncaught_else:
#                     jump gen_uncaught_raise
#     block gen_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_29)
#         jump gen_uncaught_raise
#     block gen_uncaught_raise:
#         raise _dp_uncaught_exc_29

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def gen(): ...
#         return

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         _dp_tmp_4 = Suppress()
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_4)
#         __dp_contextmanager_enter(_dp_tmp_4)
#         _dp_with_ok_2 = True
#         try_jump:
#             body_label: _dp_module_init_6
#             except_label: _dp_module_init_5
#         block _dp_module_init_6:
#             raise RuntimeError("boom")
#         block _dp_module_init_5:
#             _dp_with_ok_2 = False
#             __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#             jump _dp_module_init_4
#             block _dp_module_init_4:
#                 _dp_try_exc_2 = None
#                 jump _dp_module_init_3
#                 block _dp_module_init_3:
#                     _dp_try_exc_2 = __dp_current_exception()
#                     if_term _dp_with_ok_2:
#                         then:
#                             block _dp_bb__dp_module_init_4_then:
#                                 jump _dp_module_init_2
#                         else:
#                             block _dp_bb__dp_module_init_4_else:
#                                 jump _dp_module_init_1
#                     block _dp_module_init_2:
#                         __dp_contextmanager_exit(_dp_with_exit_1, None)
#                         jump _dp_module_init_1
#                     block _dp_module_init_1:
#                         _dp_with_exit_1 = None
#                         _dp_tmp_4 = None
#                         if_term __dp_is_not(_dp_try_exc_2, None):
#                             then:
#                                 block _dp_bb__dp_module_init_5_then:
#                                     jump _dp_module_init_0
#                             else:
#                                 block _dp_bb__dp_module_init_5_else:
#                                     jump _dp_module_init_7
#                         block _dp_module_init_0:
#                             raise _dp_try_exc_2
#                         block _dp_module_init_7:
#                             return

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# module_init: _dp_module_init

# function inner() [kind=function, bind=inner, target=local, qualname=outer.<locals>.inner]
#     block inner_start:
#         return __dp_load_cell(_dp_cell_x)

# function outer() [kind=function, bind=outer, target=module_global, qualname=outer]
#     block outer_start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         def inner(): ...
#         return inner()

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def outer(): ...
#         return

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==

# module_init: _dp_module_init

# function choose(a, b) [kind=function, bind=choose, target=module_global, qualname=choose]
#     block choose_start:
#         total = __dp_add(a, b)
#         if_term __dp_gt(total, 5):
#             then:
#                 block _dp_bb_choose_1_then:
#                     jump choose_0
#             else:
#                 block _dp_bb_choose_1_else:
#                     jump choose_1
#         block choose_0:
#             return a
#         block choose_1:
#             return b

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def choose(a, b): ...
#         return

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==

# module_init: _dp_module_init

# function inner() [kind=function, bind=inner, target=local, qualname=outer.<locals>.inner]
#     block inner_start:
#         __dp_store_cell(_dp_cell_x, 2)
#         return __dp_load_cell(_dp_cell_x)

# function outer() [kind=function, bind=outer, target=module_global, qualname=outer]
#     block outer_start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         def inner(): ...
#         return inner()

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def outer(): ...
#         return

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         try_jump:
#             body_label: _dp_module_init_3
#             except_label: _dp_module_init_2
#         block _dp_module_init_3:
#             print(1)
#             return
#         block _dp_module_init_2:
#             if_term __dp_exception_matches(__dp_current_exception(), Exception):
#                 then:
#                     block _dp_bb__dp_module_init_3_then:
#                         jump _dp_module_init_0
#                 else:
#                     block _dp_bb__dp_module_init_3_else:
#                         jump _dp_module_init_1
#             block _dp_module_init_0:
#                 print(2)
#                 return
#             block _dp_module_init_1:
#                 raise

# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")
    else:
        print("finsihed")


# ==

# module_init: _dp_module_init

# function complicated(a) [kind=generator, bind=complicated, target=module_global, qualname=complicated]
#     block complicated_dispatch:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block complicated_dispatch_then:
#                     jump complicated_dispatch_send_table
#             else:
#                 block complicated_dispatch_else:
#                     jump complicated_dispatch_throw_table
#         block complicated_dispatch_send_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [complicated_done, complicated_dispatch_send_target_1, complicated_dispatch_send_target_2] default complicated_invalid
#             block complicated_done:
#                 return
#             block complicated_dispatch_send_target_1:
#                 jump complicated_start
#                 block complicated_start:
#                     _dp_iter_1 = __dp_iter(a)
#                     jump complicated_9
#             block complicated_dispatch_send_target_2:
#                 jump complicated_5
#         block complicated_9:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_complicated_3_then:
#                         jump complicated_0
#                 else:
#                     block _dp_bb_complicated_3_else:
#                         jump complicated_8
#             block complicated_0:
#                 print("finsihed")
#                 return
#             block complicated_8:
#                 i = _dp_tmp_2
#                 _dp_tmp_2 = None
#                 jump complicated_7
#                 block complicated_7:
#                     try_jump:
#                         body_label: complicated_6
#                         except_label: complicated_3
#                     block complicated_6:
#                         j = __dp_add(i, 1)
#                         return j
#                     block complicated_3:
#                         if_term __dp_exception_matches(__dp_current_exception(), Exception):
#                             then:
#                                 block _dp_bb_complicated_9_then:
#                                     jump complicated_1
#                             else:
#                                 block _dp_bb_complicated_9_else:
#                                     jump complicated_2
#                         block complicated_1:
#                             print("oops")
#                             jump complicated_9
#                         block complicated_2:
#                             raise
#         block complicated_5:
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_complicated_14_true:
#                         jump complicated_4
#                 else:
#                     block _dp_bb_complicated_14_false:
#                         jump complicated_9
#             block complicated_4:
#                 raise _dp_resume_exc
#         block complicated_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#         block complicated_dispatch_throw_table:
#             branch_table __dp_load_cell(_dp_cell__dp_pc) -> [complicated_dispatch_throw_done, complicated_dispatch_throw_target_1, complicated_dispatch_throw_target_2] default complicated_invalid
#             block complicated_dispatch_throw_done:
#                 raise _dp_resume_exc
#             block complicated_dispatch_throw_target_1:
#                 jump complicated_dispatch_throw_unstarted
#                 block complicated_dispatch_throw_unstarted:
#                     raise _dp_resume_exc
#             block complicated_dispatch_throw_target_2:
#                 jump complicated_5
#     block complicated_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 block complicated_uncaught_then:
#                     jump complicated_uncaught_set_done
#             else:
#                 block complicated_uncaught_else:
#                     jump complicated_uncaught_raise
#     block complicated_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_15)
#         jump complicated_uncaught_raise
#     block complicated_uncaught_raise:
#         raise _dp_uncaught_exc_15

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     block _dp_module_init_start:
#         def complicated(a): ...
#         return
