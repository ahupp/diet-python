# import_simple

import a

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "a", __dp_import_("a", __spec__))
#         return

# import_dotted_alias

import a.b as c

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "c", __dp_import_attr(__dp_import_("a.b", __spec__), "b"))
#         return

# import_from_alias

from pkg.mod import name as alias

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_import_1 = __dp_import_("pkg.mod", __spec__, __dp_list(__dp_tuple("name")))
#         __dp_store_global(globals(), "alias", __dp_import_attr(_dp_import_1, "name"))
#         return

# decorator_function


@dec
def f():
    pass


# ==

# module_init: _dp_module_init

# function f()
#     kind: function
#     bind: f
#     qualname: f
#     block start:
#         pass
#         return

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "f", dec(__dp_make_function("start", 0, "f", "f", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None)))
#         return

# assign_attr

obj.x = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return

# assign_subscript

obj[i] = v

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)
#         return

# assign_tuple_unpack

a, b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#         __dp_store_global(globals(), "a", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0))
#         __dp_store_global(globals(), "b", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1))
#         _dp_tmp_1 = __dp_DELETED
#         return

# assign_star_unpack

a, *b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#         __dp_store_global(globals(), "a", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0))
#         __dp_store_global(globals(), "b", __dp_list(__dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1)))
#         _dp_tmp_1 = __dp_DELETED
#         return

# assign_multi_targets

a = b = f()

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_tmp_1 = f()
#         __dp_store_global(globals(), "a", _dp_tmp_1)
#         __dp_store_global(globals(), "b", _dp_tmp_1)
#         return

# ann_assign_simple

x: int = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "x", 1)
#         __annotate__ = __dp_exec_function_def_source('def __annotate__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_tuple=__dp_tuple):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple(("x", "int")))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple(("x", int)))', __dp_globals(), __dp_tuple(), "__annotate__")
#         __dp_store_global(globals(), "__annotate__", __dp_update_fn(__annotate__, "__annotate__", "__annotate__", None))
#         return

# ann_assign_attr

obj.x: int = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return

# aug_assign_attr

obj.x += 1

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))
#         return

# delete_mixed

del obj.x, obj[i], x

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_delattr(obj, "x")
#         __dp_delitem(obj, i)
#         __dp_delitem(globals(), "x")
#         return

# assert_no_msg

assert cond

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         if_term __debug__:
#             then:
#                 block _dp_bb__dp_module_init_1:
#                     if_term __dp_not_(cond):
#                         then:
#                             block _dp_bb__dp_module_init_0:
#                                 raise __dp_AssertionError
#                         else:
#                             jump _dp_bb__dp_module_init_2
#             else:
#                 jump _dp_bb__dp_module_init_2
#         block _dp_bb__dp_module_init_2:
#             return

# assert_with_msg

assert cond, "oops"

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         if_term __debug__:
#             then:
#                 block _dp_bb__dp_module_init_1:
#                     if_term __dp_not_(cond):
#                         then:
#                             block _dp_bb__dp_module_init_0:
#                                 raise __dp_AssertionError("oops")
#                         else:
#                             jump _dp_bb__dp_module_init_2
#             else:
#                 jump _dp_bb__dp_module_init_2
#         block _dp_bb__dp_module_init_2:
#             return

# raise_from

raise E from cause

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
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

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     entry_liveins: [_dp_try_exc_1, _dp_try_exc_4]
#     cellvars: [_dp_try_exc_1->_dp_cell__dp_try_exc_1@deleted, _dp_try_exc_4->_dp_cell__dp_try_exc_4@deleted]
#     block start:
#         try_jump:
#             body_label: _dp_bb__dp_module_init_6
#             except_label: _dp_bb__dp_module_init_5
#         block _dp_bb__dp_module_init_6:
#             f()
#             return
#         block _dp_bb__dp_module_init_5:
#             if_term __dp_exception_matches(__dp_current_exception(), E):
#                 then:
#                     block _dp_bb__dp_module_init_3:
#                         __dp_store_global(globals(), "e", __dp_current_exception())
#                         try_jump:
#                             body_label: _dp_bb__dp_module_init_2
#                             except_label: _dp_bb__dp_module_init_1
#                         block _dp_bb__dp_module_init_2:
#                             g(__dp_load_global(globals(), "e"))
#                             jump _dp_bb__dp_module_init_0
#                             block _dp_bb__dp_module_init_0:
#                                 __dp_delitem_quietly(globals(), "e")
#                                 return
#                         block _dp_bb__dp_module_init_1:
#                             __dp_delitem_quietly(globals(), "e")
#                             raise
#                 else:
#                     block _dp_bb__dp_module_init_4:
#                         h()
#                         return

# for_else

for x in it:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_iter_1 = __dp_iter(it)
#         jump _dp_bb__dp_module_init_3
#         block _dp_bb__dp_module_init_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_module_init_0:
#                         done()
#                         return
#                 else:
#                     block _dp_bb__dp_module_init_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_module_init_1
#                         block _dp_bb__dp_module_init_1:
#                             __dp_store_global(globals(), "x", x)
#                             body()
#                             jump _dp_bb__dp_module_init_3

# while_else

while cond:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         if_term cond:
#             then:
#                 block _dp_bb__dp_module_init_1:
#                     body()
#                     jump start
#             else:
#                 block _dp_bb__dp_module_init_0:
#                     done()
#                     return

# with_as

with cm as x:
    body()

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     entry_liveins: [_dp_try_exc_2]
#     cellvars: [_dp_try_exc_2->_dp_cell__dp_try_exc_2@deleted]
#     block start:
#         _dp_with_exit_4 = __dp_contextmanager_get_exit(cm)
#         _dp_with_enter_6 = __dp_contextmanager_enter(cm)
#         try_jump:
#             body_label: _dp_bb__dp_module_init_2
#             except_label: _dp_bb__dp_module_init_0
#         block _dp_bb__dp_module_init_2:
#             x = _dp_with_enter_6
#             _dp_with_enter_6 = None
#             body()
#             jump _dp_bb__dp_module_init_1
#             block _dp_bb__dp_module_init_1:
#                 _dp_try_exc_2 = None
#                 jump _dp_bb__dp_module_init_0
#         block _dp_bb__dp_module_init_0:
#             _dp_with_exit_call_5 = _dp_with_exit_4
#             _dp_with_exit_4 = None
#             _dp_with_enter_6 = None
#             __dp_contextmanager_exit(_dp_with_exit_call_5, __dp_exc_info())
#             _dp_with_exit_call_5 = None
#             return

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# module_init: _dp_module_init

# function inner()
#     kind: function
#     bind: inner
#     qualname: inner
#     block start:
#         value = 1
#         return value

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "inner", __dp_make_function("start", 0, "inner", "inner", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2)
#     kind: function
#     bind: _dp_listcomp_3
#     qualname: _dp_listcomp_3
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = __dp_list(__dp_tuple())
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_listcomp_3_3
#         block _dp_bb__dp_listcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_listcomp_3_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_listcomp_3_1
#                         block _dp_bb__dp_listcomp_3_1:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb__dp_listcomp_3_3

# function _dp_setcomp_6(_dp_iter_5)
#     kind: function
#     bind: _dp_setcomp_6
#     qualname: _dp_setcomp_6
#     display_name: <setcomp>
#     block start:
#         _dp_tmp_4 = set()
#         _dp_iter_10 = __dp_iter(_dp_iter_5)
#         jump _dp_bb__dp_setcomp_6_3
#         block _dp_bb__dp_setcomp_6_3:
#             _dp_tmp_11 = __dp_next_or_sentinel(_dp_iter_10)
#             if_term __dp_is_(_dp_tmp_11, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_setcomp_6_0:
#                         return _dp_tmp_4
#                 else:
#                     block _dp_bb__dp_setcomp_6_2:
#                         x = _dp_tmp_11
#                         _dp_tmp_11 = None
#                         jump _dp_bb__dp_setcomp_6_1
#                         block _dp_bb__dp_setcomp_6_1:
#                             _dp_tmp_4.add(x)
#                             jump _dp_bb__dp_setcomp_6_3

# function _dp_dictcomp_9(_dp_iter_8)
#     kind: function
#     bind: _dp_dictcomp_9
#     qualname: _dp_dictcomp_9
#     display_name: <dictcomp>
#     block start:
#         _dp_tmp_7 = __dp_dict()
#         _dp_iter_19 = __dp_iter(_dp_iter_8)
#         jump _dp_bb__dp_dictcomp_9_3
#         block _dp_bb__dp_dictcomp_9_3:
#             _dp_tmp_20 = __dp_next_or_sentinel(_dp_iter_19)
#             if_term __dp_is_(_dp_tmp_20, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_dictcomp_9_0:
#                         return _dp_tmp_7
#                 else:
#                     block _dp_bb__dp_dictcomp_9_2:
#                         _dp_tmp_22 = __dp_unpack(_dp_tmp_20, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_22, 0)
#                         v = __dp_getitem(_dp_tmp_22, 1)
#                         del _dp_tmp_22
#                         _dp_tmp_20 = None
#                         jump _dp_bb__dp_dictcomp_9_1
#                         block _dp_bb__dp_dictcomp_9_1:
#                             __dp_setitem(_dp_tmp_7, k, v)
#                             jump _dp_bb__dp_dictcomp_9_3

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "_dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "xs", _dp_listcomp_3(it))
#         _dp_setcomp_6 = __dp_make_function("start", 1, "<setcomp>", "_dp_setcomp_6", __dp_tuple("_dp_iter_5"), __dp_tuple(__dp_tuple("_dp_iter_5", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "ys", _dp_setcomp_6(it))
#         _dp_dictcomp_9 = __dp_make_function("start", 2, "<dictcomp>", "_dp_dictcomp_9", __dp_tuple("_dp_iter_8"), __dp_tuple(__dp_tuple("_dp_iter_8", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "zs", _dp_dictcomp_9(items))
#         return

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2)
#     kind: function
#     bind: _dp_listcomp_3
#     qualname: f.<locals>._dp_listcomp_3
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = __dp_list(__dp_tuple())
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_listcomp_3_4
#         block _dp_bb__dp_listcomp_3_4:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_listcomp_3_3:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_listcomp_3_2
#                         block _dp_bb__dp_listcomp_3_2:
#                             if_term __dp_gt(x, 0):
#                                 then:
#                                     block _dp_bb__dp_listcomp_3_1:
#                                         _dp_tmp_1.append(x)
#                                         jump _dp_bb__dp_listcomp_3_4
#                                 else:
#                                     jump _dp_bb__dp_listcomp_3_4

# function f()
#     kind: function
#     bind: f
#     qualname: f
#     block start:
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "f.<locals>._dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         return _dp_listcomp_3(it)

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "f", __dp_make_function("start", 1, "f", "f", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2)
#     kind: function
#     bind: _dp_listcomp_3
#     qualname: C._dp_listcomp_3
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = __dp_list(__dp_tuple())
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_listcomp_3_3
#         block _dp_bb__dp_listcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_listcomp_3_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_listcomp_3_1
#                         block _dp_bb__dp_listcomp_3_1:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb__dp_listcomp_3_3

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg)
#     kind: function
#     bind: _dp_class_ns_C
#     qualname: _dp_class_ns_C
#     block start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "C._dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "xs", _dp_listcomp_3(__dp_class_lookup_global(_dp_class_ns, "it", globals())))
#         return

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None)
#     kind: function
#     bind: _dp_define_class_C
#     qualname: _dp_define_class_C
#     block start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_class_ns_C = __dp_make_function("start", 1, "_dp_class_ns_C", "_dp_class_ns_C", __dp_tuple("_dp_class_ns", "_dp_classcell_arg"), __dp_tuple(__dp_tuple("_dp_class_ns", None, __dp__.NO_DEFAULT), __dp_tuple("_dp_classcell_arg", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         _dp_define_class_C = __dp_make_function("start", 2, "_dp_define_class_C", "_dp_define_class_C", __dp_tuple("_dp_class_ns_fn", "_dp_class_ns_outer", "_dp_prepare_dict"), __dp_tuple(__dp_tuple("_dp_class_ns_fn", None, __dp__.NO_DEFAULT), __dp_tuple("_dp_class_ns_outer", None, __dp__.NO_DEFAULT), __dp_tuple("_dp_prepare_dict", None, None)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))
#         return

# with_multi

with a as x, b as y:
    body()

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     entry_liveins: [_dp_try_exc_2, _dp_try_exc_10]
#     cellvars: [_dp_try_exc_2->_dp_cell__dp_try_exc_2@deleted, _dp_try_exc_10->_dp_cell__dp_try_exc_10@deleted]
#     block start:
#         _dp_with_exit_4 = __dp_contextmanager_get_exit(a)
#         _dp_with_enter_6 = __dp_contextmanager_enter(a)
#         try_jump:
#             body_label: _dp_bb__dp_module_init_7
#             except_label: _dp_bb__dp_module_init_0
#         block _dp_bb__dp_module_init_7:
#             x = _dp_with_enter_6
#             _dp_with_enter_6 = None
#             jump _dp_bb__dp_module_init_6
#             block _dp_bb__dp_module_init_6:
#                 _dp_with_exit_12 = __dp_contextmanager_get_exit(b)
#                 _dp_with_enter_14 = __dp_contextmanager_enter(b)
#                 try_jump:
#                     body_label: _dp_bb__dp_module_init_5
#                     except_label: _dp_bb__dp_module_init_2
#                 block _dp_bb__dp_module_init_5:
#                     y = _dp_with_enter_14
#                     _dp_with_enter_14 = None
#                     jump _dp_bb__dp_module_init_4
#                     block _dp_bb__dp_module_init_4:
#                         body()
#                         jump _dp_bb__dp_module_init_3
#                         block _dp_bb__dp_module_init_3:
#                             _dp_try_exc_10 = None
#                             jump _dp_bb__dp_module_init_2
#                 block _dp_bb__dp_module_init_2:
#                     _dp_with_exit_call_13 = _dp_with_exit_12
#                     _dp_with_exit_12 = None
#                     _dp_with_enter_14 = None
#                     __dp_contextmanager_exit(_dp_with_exit_call_13, __dp_exc_info())
#                     _dp_with_exit_call_13 = None
#                     jump _dp_bb__dp_module_init_1
#                     block _dp_bb__dp_module_init_1:
#                         _dp_try_exc_2 = None
#                         jump _dp_bb__dp_module_init_0
#         block _dp_bb__dp_module_init_0:
#             _dp_with_exit_call_5 = _dp_with_exit_4
#             _dp_with_exit_4 = None
#             _dp_with_enter_6 = None
#             __dp_contextmanager_exit(_dp_with_exit_call_5, __dp_exc_info())
#             _dp_with_exit_call_5 = None
#             return

# async_for


async def run():
    async for x in ait:
        body()


# ==

# module_init: _dp_module_init

# function run()
#     kind: function
#     bind: run
#     qualname: run
#     local_cell_slots: [_dp_cell__dp_iter_2, _dp_cell__dp_pc, _dp_cell__dp_tmp_3, _dp_cell__dp_try_exc_11, _dp_cell__dp_yield_from_close_14, _dp_cell__dp_yield_from_exc_12, _dp_cell__dp_yield_from_iter_7, _dp_cell__dp_yield_from_raise_13, _dp_cell__dp_yield_from_result_10, _dp_cell__dp_yield_from_sent_9, _dp_cell__dp_yield_from_throw_15, _dp_cell__dp_yield_from_y_8, _dp_cell__dp_yieldfrom, _dp_cell_x]
#     cellvars: [_dp_iter_2->_dp_cell__dp_iter_2@deferred, _dp_yield_from_iter_7->_dp_cell__dp_yield_from_iter_7@deferred, _dp_yield_from_y_8->_dp_cell__dp_yield_from_y_8@deferred, _dp_try_exc_11->_dp_cell__dp_try_exc_11@deleted, _dp_yield_from_result_10->_dp_cell__dp_yield_from_result_10@deferred, _dp_yield_from_raise_13->_dp_cell__dp_yield_from_raise_13@deferred, _dp_yield_from_exc_12->_dp_cell__dp_yield_from_exc_12@deferred, _dp_yield_from_sent_9->_dp_cell__dp_yield_from_sent_9@deferred, _dp_yield_from_close_14->_dp_cell__dp_yield_from_close_14@deferred, _dp_yield_from_throw_15->_dp_cell__dp_yield_from_throw_15@deferred, _dp_tmp_3->_dp_cell__dp_tmp_3@deferred, x->_dp_cell_x@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         _dp_cell__dp_iter_2 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_iter_7 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_y_8 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_11 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_yield_from_result_10 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_raise_13 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_exc_12 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_sent_9 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_close_14 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_throw_15 = __dp_make_cell(None)
#         _dp_cell__dp_tmp_3 = __dp_make_cell(None)
#         _dp_cell_x = __dp_make_cell(None)
#         _dp_cell__dp_pc = __dp_make_cell(1)
#         _dp_cell__dp_yieldfrom = __dp_make_cell(None)
#         return __dp_make_coroutine_from_generator(__dp_make_closure_generator(__dp_def_hidden_resume_fn("start", 1, "_dp_resume", "run", __dp_tuple("_dp_self", "_dp_send_value", "_dp_resume_exc", "_dp_cell__dp_iter_2", "_dp_cell__dp_yield_from_iter_7", "_dp_cell__dp_yield_from_y_8", "_dp_cell__dp_try_exc_11", "_dp_cell__dp_yield_from_result_10", "_dp_cell__dp_yield_from_raise_13", "_dp_cell__dp_yield_from_exc_12", "_dp_cell__dp_yield_from_sent_9", "_dp_cell__dp_yield_from_close_14", "_dp_cell__dp_yield_from_throw_15", "_dp_cell__dp_tmp_3", "_dp_cell_x", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple("_dp_cell__dp_iter_2", "_dp_cell__dp_yield_from_iter_7", "_dp_cell__dp_yield_from_y_8", "_dp_cell__dp_try_exc_11", "_dp_cell__dp_yield_from_result_10", "_dp_cell__dp_yield_from_raise_13", "_dp_cell__dp_yield_from_exc_12", "_dp_cell__dp_yield_from_sent_9", "_dp_cell__dp_yield_from_close_14", "_dp_cell__dp_yield_from_throw_15", "_dp_cell__dp_tmp_3", "_dp_cell_x", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple(_dp_cell__dp_iter_2, _dp_cell__dp_yield_from_iter_7, _dp_cell__dp_yield_from_y_8, _dp_cell__dp_try_exc_11, _dp_cell__dp_yield_from_result_10, _dp_cell__dp_yield_from_raise_13, _dp_cell__dp_yield_from_exc_12, _dp_cell__dp_yield_from_sent_9, _dp_cell__dp_yield_from_close_14, _dp_cell__dp_yield_from_throw_15, _dp_cell__dp_tmp_3, _dp_cell_x, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom), __dp_globals(), __name__, async_gen=False), "run", "run"))

# function run_resume()
#     kind: generator
#     bind: run_resume
#     qualname: run
#     display_name: _dp_resume
#     entry_liveins: [_dp_self, _dp_send_value, _dp_resume_exc, _dp_cell__dp_iter_2, _dp_cell__dp_yield_from_iter_7, _dp_cell__dp_yield_from_y_8, _dp_cell__dp_try_exc_11, _dp_cell__dp_yield_from_result_10, _dp_cell__dp_yield_from_raise_13, _dp_cell__dp_yield_from_exc_12, _dp_cell__dp_yield_from_sent_9, _dp_cell__dp_yield_from_close_14, _dp_cell__dp_yield_from_throw_15, _dp_cell__dp_tmp_3, _dp_cell_x, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     local_cell_slots: [_dp_cell__dp_iter_2, _dp_cell__dp_pc, _dp_cell__dp_tmp_3, _dp_cell__dp_try_exc_11, _dp_cell__dp_yield_from_close_14, _dp_cell__dp_yield_from_exc_12, _dp_cell__dp_yield_from_iter_7, _dp_cell__dp_yield_from_raise_13, _dp_cell__dp_yield_from_result_10, _dp_cell__dp_yield_from_sent_9, _dp_cell__dp_yield_from_throw_15, _dp_cell__dp_yield_from_y_8, _dp_cell__dp_yieldfrom, _dp_cell_x]
#     cellvars: [_dp_iter_2->_dp_cell__dp_iter_2@deferred, _dp_yield_from_iter_7->_dp_cell__dp_yield_from_iter_7@deferred, _dp_yield_from_y_8->_dp_cell__dp_yield_from_y_8@deferred, _dp_try_exc_11->_dp_cell__dp_try_exc_11@deleted, _dp_yield_from_result_10->_dp_cell__dp_yield_from_result_10@deferred, _dp_yield_from_raise_13->_dp_cell__dp_yield_from_raise_13@deferred, _dp_yield_from_exc_12->_dp_cell__dp_yield_from_exc_12@deferred, _dp_yield_from_sent_9->_dp_cell__dp_yield_from_sent_9@deferred, _dp_yield_from_close_14->_dp_cell__dp_yield_from_close_14@deferred, _dp_yield_from_throw_15->_dp_cell__dp_yield_from_throw_15@deferred, _dp_tmp_3->_dp_cell__dp_tmp_3@deferred, x->_dp_cell_x@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block _dp_bb_run_dispatch_send_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_run_done, _dp_bb_run_start, _dp_bb_run_7, _dp_bb_run_26] default _dp_bb_run_invalid
#                     block _dp_bb_run_done:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_run_done_return_done
#                         block _dp_bb_run_done_return_done:
#                             raise StopIteration()
#                     block _dp_bb_run_start:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         jump _dp_bb_run_24
#             else:
#                 block _dp_bb_run_dispatch_throw_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_run_dispatch_throw_done, _dp_bb_run_dispatch_throw_unstarted, _dp_bb_run_7, _dp_bb_run_26] default _dp_bb_run_invalid
#                     block _dp_bb_run_dispatch_throw_done:
#                         raise _dp_resume_exc
#                     block _dp_bb_run_dispatch_throw_unstarted:
#                         raise _dp_resume_exc
#         block _dp_bb_run_24:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             _dp_iter_2 = __dp_aiter(ait)
#             __dp_store_cell(_dp_cell__dp_iter_2, _dp_iter_2)
#             jump _dp_bb_run_19
#         block _dp_bb_run_19:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             jump _dp_bb_run_0
#             block _dp_bb_run_0:
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                 _dp_iter_2 = __dp_load_deleted_name("_dp_iter_2", __dp_load_cell(_dp_cell__dp_iter_2))
#                 __dp_store_cell(_dp_cell__dp_iter_2, _dp_iter_2)
#                 _dp_yield_from_iter_7 = iter(__dp_await_iter(__dp_anext_or_sentinel(_dp_iter_2)))
#                 __dp_store_cell(_dp_cell__dp_yield_from_iter_7, _dp_yield_from_iter_7)
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_7)
#                 try_jump:
#                     body_label: _dp_bb_run_1
#                     except_label: _dp_bb_run_2
#         block _dp_bb_run_1:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             _dp_yield_from_y_8 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_8, _dp_yield_from_y_8)
#             jump _dp_bb_run_6
#         block _dp_bb_run_6:
#             _dp_yield_from_y_8 = __dp_load_deleted_name("_dp_yield_from_y_8", __dp_load_cell(_dp_cell__dp_yield_from_y_8))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_8, _dp_yield_from_y_8)
#             __dp_store_cell(_dp_cell__dp_pc, 2)
#             return _dp_yield_from_y_8
#         block _dp_bb_run_2:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             _dp_try_exc_11 = __dp_load_deleted_name("_dp_try_exc_11", __dp_load_cell(_dp_cell__dp_try_exc_11))
#             __dp_store_cell(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             _dp_try_exc_11 = __dp_current_exception()
#             __dp_store_cell(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             if_term __dp_exception_matches(_dp_try_exc_11, StopIteration):
#                 then:
#                     block _dp_bb_run_3:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         _dp_try_exc_11 = __dp_load_deleted_name("_dp_try_exc_11", __dp_load_cell(_dp_cell__dp_try_exc_11))
#                         __dp_store_cell(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         _dp_yield_from_result_10 = _dp_try_exc_11.value
#                         __dp_store_cell(_dp_cell__dp_yield_from_result_10, _dp_yield_from_result_10)
#                         jump _dp_bb_run_4
#                         block _dp_bb_run_4:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                             jump _dp_bb_run_20
#                             block _dp_bb_run_20:
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                 _dp_yield_from_result_10 = __dp_load_deleted_name("_dp_yield_from_result_10", __dp_load_cell(_dp_cell__dp_yield_from_result_10))
#                                 __dp_store_cell(_dp_cell__dp_yield_from_result_10, _dp_yield_from_result_10)
#                                 _dp_tmp_3 = _dp_yield_from_result_10
#                                 __dp_store_cell(_dp_cell__dp_tmp_3, _dp_tmp_3)
#                                 jump _dp_bb_run_23
#                                 block _dp_bb_run_23:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                     _dp_tmp_3 = __dp_load_deleted_name("_dp_tmp_3", __dp_load_cell(_dp_cell__dp_tmp_3))
#                                     __dp_store_cell(_dp_cell__dp_tmp_3, _dp_tmp_3)
#                                     if_term __dp_is_(_dp_tmp_3, __dp__.ITER_COMPLETE):
#                                         then:
#                                             block _dp_bb_run_27:
#                                                 __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                                                 jump _dp_bb_run_27_return_done
#                                                 block _dp_bb_run_27_return_done:
#                                                     raise StopIteration()
#                                         else:
#                                             block _dp_bb_run_22:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                                 _dp_tmp_3 = __dp_load_deleted_name("_dp_tmp_3", __dp_load_cell(_dp_cell__dp_tmp_3))
#                                                 __dp_store_cell(_dp_cell__dp_tmp_3, _dp_tmp_3)
#                                                 x = _dp_tmp_3
#                                                 __dp_store_cell(_dp_cell_x, x)
#                                                 _dp_tmp_3 = None
#                                                 __dp_store_cell(_dp_cell__dp_tmp_3, _dp_tmp_3)
#                                                 jump _dp_bb_run_21
#                                                 block _dp_bb_run_21:
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                                     body()
#                                                     jump _dp_bb_run_19
#                 else:
#                     block _dp_bb_run_5:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         _dp_try_exc_11 = __dp_load_deleted_name("_dp_try_exc_11", __dp_load_cell(_dp_cell__dp_try_exc_11))
#                         __dp_store_cell(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         _dp_yield_from_raise_13 = _dp_try_exc_11
#                         __dp_store_cell(_dp_cell__dp_yield_from_raise_13, _dp_yield_from_raise_13)
#                         jump _dp_bb_run_12
#         block _dp_bb_run_12:
#             _dp_yield_from_raise_13 = __dp_load_deleted_name("_dp_yield_from_raise_13", __dp_load_cell(_dp_cell__dp_yield_from_raise_13))
#             __dp_store_cell(_dp_cell__dp_yield_from_raise_13, _dp_yield_from_raise_13)
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_13
#         block _dp_bb_run_7:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             _dp_yield_from_sent_9 = _dp_send_value
#             __dp_store_cell(_dp_cell__dp_yield_from_sent_9, _dp_yield_from_sent_9)
#             _dp_yield_from_exc_12 = _dp_resume_exc
#             __dp_store_cell(_dp_cell__dp_yield_from_exc_12, _dp_yield_from_exc_12)
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_12, None):
#                 then:
#                     block _dp_bb_run_8:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         _dp_yield_from_exc_12 = __dp_load_deleted_name("_dp_yield_from_exc_12", __dp_load_cell(_dp_cell__dp_yield_from_exc_12))
#                         __dp_store_cell(_dp_cell__dp_yield_from_exc_12, _dp_yield_from_exc_12)
#                         if_term __dp_exception_matches(_dp_yield_from_exc_12, GeneratorExit):
#                             then:
#                                 block _dp_bb_run_9:
#                                     _dp_yield_from_close_14 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_close_14, _dp_yield_from_close_14)
#                                     if_term __dp_is_not(_dp_yield_from_close_14, None):
#                                         then:
#                                             block _dp_bb_run_10:
#                                                 _dp_yield_from_close_14 = __dp_load_deleted_name("_dp_yield_from_close_14", __dp_load_cell(_dp_cell__dp_yield_from_close_14))
#                                                 __dp_store_cell(_dp_cell__dp_yield_from_close_14, _dp_yield_from_close_14)
#                                                 _dp_yield_from_close_14()
#                                                 jump _dp_bb_run_11
#                                         else:
#                                             jump _dp_bb_run_11
#                             else:
#                                 block _dp_bb_run_13:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                     _dp_yield_from_throw_15 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_15, _dp_yield_from_throw_15)
#                                     if_term __dp_is_(_dp_yield_from_throw_15, None):
#                                         then:
#                                             jump _dp_bb_run_11
#                                         else:
#                                             block _dp_bb_run_14:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                                 try_jump:
#                                                     body_label: _dp_bb_run_15
#                                                     except_label: _dp_bb_run_2
#                                                 block _dp_bb_run_15:
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                                     _dp_yield_from_exc_12 = __dp_load_deleted_name("_dp_yield_from_exc_12", __dp_load_cell(_dp_cell__dp_yield_from_exc_12))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_exc_12, _dp_yield_from_exc_12)
#                                                     _dp_yield_from_throw_15 = __dp_load_deleted_name("_dp_yield_from_throw_15", __dp_load_cell(_dp_cell__dp_yield_from_throw_15))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_15, _dp_yield_from_throw_15)
#                                                     _dp_yield_from_y_8 = _dp_yield_from_throw_15(_dp_yield_from_exc_12)
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_y_8, _dp_yield_from_y_8)
#                                                     jump _dp_bb_run_6
#                         block _dp_bb_run_11:
#                             _dp_yield_from_exc_12 = __dp_load_deleted_name("_dp_yield_from_exc_12", __dp_load_cell(_dp_cell__dp_yield_from_exc_12))
#                             __dp_store_cell(_dp_cell__dp_yield_from_exc_12, _dp_yield_from_exc_12)
#                             _dp_yield_from_raise_13 = _dp_yield_from_exc_12
#                             __dp_store_cell(_dp_cell__dp_yield_from_raise_13, _dp_yield_from_raise_13)
#                             jump _dp_bb_run_12
#                 else:
#                     block _dp_bb_run_16:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                         try_jump:
#                             body_label: _dp_bb_run_17
#                             except_label: _dp_bb_run_2
#                         block _dp_bb_run_17:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                             _dp_yield_from_sent_9 = __dp_load_deleted_name("_dp_yield_from_sent_9", __dp_load_cell(_dp_cell__dp_yield_from_sent_9))
#                             __dp_store_cell(_dp_cell__dp_yield_from_sent_9, _dp_yield_from_sent_9)
#                             if_term __dp_is_(_dp_yield_from_sent_9, None):
#                                 then:
#                                     jump _dp_bb_run_1
#                                 else:
#                                     block _dp_bb_run_18:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#                                         _dp_yield_from_sent_9 = __dp_load_deleted_name("_dp_yield_from_sent_9", __dp_load_cell(_dp_cell__dp_yield_from_sent_9))
#                                         __dp_store_cell(_dp_cell__dp_yield_from_sent_9, _dp_yield_from_sent_9)
#                                         _dp_yield_from_y_8 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_9)
#                                         __dp_store_cell(_dp_cell__dp_yield_from_y_8, _dp_yield_from_y_8)
#                                         jump _dp_bb_run_6
#         block _dp_bb_run_26:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_11, _dp_try_exc_11)
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_run_25:
#                         raise _dp_resume_exc
#                 else:
#                     jump _dp_bb_run_24
#         block _dp_bb_run_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     block _dp_bb_run_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 jump _dp_bb_run_uncaught_set_done
#             else:
#                 jump _dp_bb_run_uncaught_raise
#     block _dp_bb_run_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_store_cell(_dp_cell__dp_iter_2, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_iter_7, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_y_8, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_11, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_result_10, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_raise_13, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_exc_12, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_sent_9, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_close_14, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_throw_15, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_tmp_3, __dp_DELETED)
#         __dp_store_cell(_dp_cell_x, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, __dp_DELETED)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_41)
#         jump _dp_bb_run_uncaught_raise
#     block _dp_bb_run_uncaught_raise:
#         raise _dp_uncaught_exc_41

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "run", __dp_mark_coroutine_function(__dp_make_function("start", 0, "run", "run", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None)))
#         return

# async_with


async def run():
    async with cm as x:
        body()


# ==

# module_init: _dp_module_init

# function run()
#     kind: function
#     bind: run
#     qualname: run
#     local_cell_slots: [_dp_cell__dp_pc, _dp_cell__dp_try_exc_12, _dp_cell__dp_try_exc_2, _dp_cell__dp_try_exc_3, _dp_cell__dp_try_exc_44, _dp_cell__dp_try_exc_75, _dp_cell__dp_with_exit_1, _dp_cell__dp_with_ok_2, _dp_cell__dp_with_reraise_3, _dp_cell__dp_yield_from_close_15, _dp_cell__dp_yield_from_close_47, _dp_cell__dp_yield_from_close_78, _dp_cell__dp_yield_from_exc_13, _dp_cell__dp_yield_from_exc_45, _dp_cell__dp_yield_from_exc_76, _dp_cell__dp_yield_from_iter_40, _dp_cell__dp_yield_from_iter_71, _dp_cell__dp_yield_from_iter_9, _dp_cell__dp_yield_from_raise_14, _dp_cell__dp_yield_from_raise_46, _dp_cell__dp_yield_from_raise_77, _dp_cell__dp_yield_from_result_43, _dp_cell__dp_yield_from_result_74, _dp_cell__dp_yield_from_sent_11, _dp_cell__dp_yield_from_sent_42, _dp_cell__dp_yield_from_sent_73, _dp_cell__dp_yield_from_throw_16, _dp_cell__dp_yield_from_throw_48, _dp_cell__dp_yield_from_throw_79, _dp_cell__dp_yield_from_y_10, _dp_cell__dp_yield_from_y_41, _dp_cell__dp_yield_from_y_72, _dp_cell__dp_yieldfrom, _dp_cell_x]
#     cellvars: [_dp_try_exc_3->_dp_cell__dp_try_exc_3@deleted, _dp_with_exit_1->_dp_cell__dp_with_exit_1@deferred, _dp_yield_from_iter_9->_dp_cell__dp_yield_from_iter_9@deferred, _dp_yield_from_y_10->_dp_cell__dp_yield_from_y_10@deferred, _dp_try_exc_12->_dp_cell__dp_try_exc_12@deleted, _dp_yield_from_raise_14->_dp_cell__dp_yield_from_raise_14@deferred, _dp_yield_from_exc_13->_dp_cell__dp_yield_from_exc_13@deferred, _dp_yield_from_sent_11->_dp_cell__dp_yield_from_sent_11@deferred, _dp_yield_from_close_15->_dp_cell__dp_yield_from_close_15@deferred, _dp_yield_from_throw_16->_dp_cell__dp_yield_from_throw_16@deferred, _dp_with_ok_2->_dp_cell__dp_with_ok_2@deferred, _dp_try_exc_2->_dp_cell__dp_try_exc_2@deleted, _dp_with_reraise_3->_dp_cell__dp_with_reraise_3@deferred, _dp_yield_from_iter_40->_dp_cell__dp_yield_from_iter_40@deferred, _dp_yield_from_y_41->_dp_cell__dp_yield_from_y_41@deferred, _dp_try_exc_44->_dp_cell__dp_try_exc_44@deleted, _dp_yield_from_result_43->_dp_cell__dp_yield_from_result_43@deferred, _dp_yield_from_raise_46->_dp_cell__dp_yield_from_raise_46@deferred, _dp_yield_from_exc_45->_dp_cell__dp_yield_from_exc_45@deferred, _dp_yield_from_sent_42->_dp_cell__dp_yield_from_sent_42@deferred, _dp_yield_from_close_47->_dp_cell__dp_yield_from_close_47@deferred, _dp_yield_from_throw_48->_dp_cell__dp_yield_from_throw_48@deferred, _dp_yield_from_iter_71->_dp_cell__dp_yield_from_iter_71@deferred, _dp_yield_from_y_72->_dp_cell__dp_yield_from_y_72@deferred, _dp_try_exc_75->_dp_cell__dp_try_exc_75@deleted, _dp_yield_from_result_74->_dp_cell__dp_yield_from_result_74@deferred, _dp_yield_from_raise_77->_dp_cell__dp_yield_from_raise_77@deferred, _dp_yield_from_exc_76->_dp_cell__dp_yield_from_exc_76@deferred, _dp_yield_from_sent_73->_dp_cell__dp_yield_from_sent_73@deferred, _dp_yield_from_close_78->_dp_cell__dp_yield_from_close_78@deferred, _dp_yield_from_throw_79->_dp_cell__dp_yield_from_throw_79@deferred, x->_dp_cell_x@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         _dp_cell__dp_try_exc_3 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_with_exit_1 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_iter_9 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_y_10 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_12 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_yield_from_raise_14 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_exc_13 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_sent_11 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_close_15 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_throw_16 = __dp_make_cell(None)
#         _dp_cell__dp_with_ok_2 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_2 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_with_reraise_3 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_iter_40 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_y_41 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_44 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_yield_from_result_43 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_raise_46 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_exc_45 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_sent_42 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_close_47 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_throw_48 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_iter_71 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_y_72 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_75 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_yield_from_result_74 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_raise_77 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_exc_76 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_sent_73 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_close_78 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_throw_79 = __dp_make_cell(None)
#         _dp_cell_x = __dp_make_cell(None)
#         _dp_cell__dp_pc = __dp_make_cell(1)
#         _dp_cell__dp_yieldfrom = __dp_make_cell(None)
#         return __dp_make_coroutine_from_generator(__dp_make_closure_generator(__dp_def_hidden_resume_fn("start", 1, "_dp_resume", "run", __dp_tuple("_dp_self", "_dp_send_value", "_dp_resume_exc", "_dp_cell__dp_try_exc_3", "_dp_cell__dp_with_exit_1", "_dp_cell__dp_yield_from_iter_9", "_dp_cell__dp_yield_from_y_10", "_dp_cell__dp_try_exc_12", "_dp_cell__dp_yield_from_raise_14", "_dp_cell__dp_yield_from_exc_13", "_dp_cell__dp_yield_from_sent_11", "_dp_cell__dp_yield_from_close_15", "_dp_cell__dp_yield_from_throw_16", "_dp_cell__dp_with_ok_2", "_dp_cell__dp_try_exc_2", "_dp_cell__dp_with_reraise_3", "_dp_cell__dp_yield_from_iter_40", "_dp_cell__dp_yield_from_y_41", "_dp_cell__dp_try_exc_44", "_dp_cell__dp_yield_from_result_43", "_dp_cell__dp_yield_from_raise_46", "_dp_cell__dp_yield_from_exc_45", "_dp_cell__dp_yield_from_sent_42", "_dp_cell__dp_yield_from_close_47", "_dp_cell__dp_yield_from_throw_48", "_dp_cell__dp_yield_from_iter_71", "_dp_cell__dp_yield_from_y_72", "_dp_cell__dp_try_exc_75", "_dp_cell__dp_yield_from_result_74", "_dp_cell__dp_yield_from_raise_77", "_dp_cell__dp_yield_from_exc_76", "_dp_cell__dp_yield_from_sent_73", "_dp_cell__dp_yield_from_close_78", "_dp_cell__dp_yield_from_throw_79", "_dp_cell_x", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple("_dp_cell__dp_try_exc_3", "_dp_cell__dp_with_exit_1", "_dp_cell__dp_yield_from_iter_9", "_dp_cell__dp_yield_from_y_10", "_dp_cell__dp_try_exc_12", "_dp_cell__dp_yield_from_raise_14", "_dp_cell__dp_yield_from_exc_13", "_dp_cell__dp_yield_from_sent_11", "_dp_cell__dp_yield_from_close_15", "_dp_cell__dp_yield_from_throw_16", "_dp_cell__dp_with_ok_2", "_dp_cell__dp_try_exc_2", "_dp_cell__dp_with_reraise_3", "_dp_cell__dp_yield_from_iter_40", "_dp_cell__dp_yield_from_y_41", "_dp_cell__dp_try_exc_44", "_dp_cell__dp_yield_from_result_43", "_dp_cell__dp_yield_from_raise_46", "_dp_cell__dp_yield_from_exc_45", "_dp_cell__dp_yield_from_sent_42", "_dp_cell__dp_yield_from_close_47", "_dp_cell__dp_yield_from_throw_48", "_dp_cell__dp_yield_from_iter_71", "_dp_cell__dp_yield_from_y_72", "_dp_cell__dp_try_exc_75", "_dp_cell__dp_yield_from_result_74", "_dp_cell__dp_yield_from_raise_77", "_dp_cell__dp_yield_from_exc_76", "_dp_cell__dp_yield_from_sent_73", "_dp_cell__dp_yield_from_close_78", "_dp_cell__dp_yield_from_throw_79", "_dp_cell_x", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple(_dp_cell__dp_try_exc_3, _dp_cell__dp_with_exit_1, _dp_cell__dp_yield_from_iter_9, _dp_cell__dp_yield_from_y_10, _dp_cell__dp_try_exc_12, _dp_cell__dp_yield_from_raise_14, _dp_cell__dp_yield_from_exc_13, _dp_cell__dp_yield_from_sent_11, _dp_cell__dp_yield_from_close_15, _dp_cell__dp_yield_from_throw_16, _dp_cell__dp_with_ok_2, _dp_cell__dp_try_exc_2, _dp_cell__dp_with_reraise_3, _dp_cell__dp_yield_from_iter_40, _dp_cell__dp_yield_from_y_41, _dp_cell__dp_try_exc_44, _dp_cell__dp_yield_from_result_43, _dp_cell__dp_yield_from_raise_46, _dp_cell__dp_yield_from_exc_45, _dp_cell__dp_yield_from_sent_42, _dp_cell__dp_yield_from_close_47, _dp_cell__dp_yield_from_throw_48, _dp_cell__dp_yield_from_iter_71, _dp_cell__dp_yield_from_y_72, _dp_cell__dp_try_exc_75, _dp_cell__dp_yield_from_result_74, _dp_cell__dp_yield_from_raise_77, _dp_cell__dp_yield_from_exc_76, _dp_cell__dp_yield_from_sent_73, _dp_cell__dp_yield_from_close_78, _dp_cell__dp_yield_from_throw_79, _dp_cell_x, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom), __dp_globals(), __name__, async_gen=False), "run", "run"))

# function run_resume()
#     kind: generator
#     bind: run_resume
#     qualname: run
#     display_name: _dp_resume
#     entry_liveins: [_dp_self, _dp_send_value, _dp_resume_exc, _dp_cell__dp_try_exc_3, _dp_cell__dp_with_exit_1, _dp_cell__dp_yield_from_iter_9, _dp_cell__dp_yield_from_y_10, _dp_cell__dp_try_exc_12, _dp_cell__dp_yield_from_raise_14, _dp_cell__dp_yield_from_exc_13, _dp_cell__dp_yield_from_sent_11, _dp_cell__dp_yield_from_close_15, _dp_cell__dp_yield_from_throw_16, _dp_cell__dp_with_ok_2, _dp_cell__dp_try_exc_2, _dp_cell__dp_with_reraise_3, _dp_cell__dp_yield_from_iter_40, _dp_cell__dp_yield_from_y_41, _dp_cell__dp_try_exc_44, _dp_cell__dp_yield_from_result_43, _dp_cell__dp_yield_from_raise_46, _dp_cell__dp_yield_from_exc_45, _dp_cell__dp_yield_from_sent_42, _dp_cell__dp_yield_from_close_47, _dp_cell__dp_yield_from_throw_48, _dp_cell__dp_yield_from_iter_71, _dp_cell__dp_yield_from_y_72, _dp_cell__dp_try_exc_75, _dp_cell__dp_yield_from_result_74, _dp_cell__dp_yield_from_raise_77, _dp_cell__dp_yield_from_exc_76, _dp_cell__dp_yield_from_sent_73, _dp_cell__dp_yield_from_close_78, _dp_cell__dp_yield_from_throw_79, _dp_cell_x, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     local_cell_slots: [_dp_cell__dp_pc, _dp_cell__dp_try_exc_12, _dp_cell__dp_try_exc_2, _dp_cell__dp_try_exc_3, _dp_cell__dp_try_exc_44, _dp_cell__dp_try_exc_75, _dp_cell__dp_with_exit_1, _dp_cell__dp_with_ok_2, _dp_cell__dp_with_reraise_3, _dp_cell__dp_yield_from_close_15, _dp_cell__dp_yield_from_close_47, _dp_cell__dp_yield_from_close_78, _dp_cell__dp_yield_from_exc_13, _dp_cell__dp_yield_from_exc_45, _dp_cell__dp_yield_from_exc_76, _dp_cell__dp_yield_from_iter_40, _dp_cell__dp_yield_from_iter_71, _dp_cell__dp_yield_from_iter_9, _dp_cell__dp_yield_from_raise_14, _dp_cell__dp_yield_from_raise_46, _dp_cell__dp_yield_from_raise_77, _dp_cell__dp_yield_from_result_43, _dp_cell__dp_yield_from_result_74, _dp_cell__dp_yield_from_sent_11, _dp_cell__dp_yield_from_sent_42, _dp_cell__dp_yield_from_sent_73, _dp_cell__dp_yield_from_throw_16, _dp_cell__dp_yield_from_throw_48, _dp_cell__dp_yield_from_throw_79, _dp_cell__dp_yield_from_y_10, _dp_cell__dp_yield_from_y_41, _dp_cell__dp_yield_from_y_72, _dp_cell__dp_yieldfrom, _dp_cell_x]
#     cellvars: [_dp_try_exc_3->_dp_cell__dp_try_exc_3@deleted, _dp_with_exit_1->_dp_cell__dp_with_exit_1@deferred, _dp_yield_from_iter_9->_dp_cell__dp_yield_from_iter_9@deferred, _dp_yield_from_y_10->_dp_cell__dp_yield_from_y_10@deferred, _dp_try_exc_12->_dp_cell__dp_try_exc_12@deleted, _dp_yield_from_raise_14->_dp_cell__dp_yield_from_raise_14@deferred, _dp_yield_from_exc_13->_dp_cell__dp_yield_from_exc_13@deferred, _dp_yield_from_sent_11->_dp_cell__dp_yield_from_sent_11@deferred, _dp_yield_from_close_15->_dp_cell__dp_yield_from_close_15@deferred, _dp_yield_from_throw_16->_dp_cell__dp_yield_from_throw_16@deferred, _dp_with_ok_2->_dp_cell__dp_with_ok_2@deferred, _dp_try_exc_2->_dp_cell__dp_try_exc_2@deleted, _dp_with_reraise_3->_dp_cell__dp_with_reraise_3@deferred, _dp_yield_from_iter_40->_dp_cell__dp_yield_from_iter_40@deferred, _dp_yield_from_y_41->_dp_cell__dp_yield_from_y_41@deferred, _dp_try_exc_44->_dp_cell__dp_try_exc_44@deleted, _dp_yield_from_result_43->_dp_cell__dp_yield_from_result_43@deferred, _dp_yield_from_raise_46->_dp_cell__dp_yield_from_raise_46@deferred, _dp_yield_from_exc_45->_dp_cell__dp_yield_from_exc_45@deferred, _dp_yield_from_sent_42->_dp_cell__dp_yield_from_sent_42@deferred, _dp_yield_from_close_47->_dp_cell__dp_yield_from_close_47@deferred, _dp_yield_from_throw_48->_dp_cell__dp_yield_from_throw_48@deferred, _dp_yield_from_iter_71->_dp_cell__dp_yield_from_iter_71@deferred, _dp_yield_from_y_72->_dp_cell__dp_yield_from_y_72@deferred, _dp_try_exc_75->_dp_cell__dp_try_exc_75@deleted, _dp_yield_from_result_74->_dp_cell__dp_yield_from_result_74@deferred, _dp_yield_from_raise_77->_dp_cell__dp_yield_from_raise_77@deferred, _dp_yield_from_exc_76->_dp_cell__dp_yield_from_exc_76@deferred, _dp_yield_from_sent_73->_dp_cell__dp_yield_from_sent_73@deferred, _dp_yield_from_close_78->_dp_cell__dp_yield_from_close_78@deferred, _dp_yield_from_throw_79->_dp_cell__dp_yield_from_throw_79@deferred, x->_dp_cell_x@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block _dp_bb_run_dispatch_send_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_run_done, _dp_bb_run_start, _dp_bb_run_9, _dp_bb_run_33, _dp_bb_run_56, _dp_bb_run_71] default _dp_bb_run_invalid
#                     block _dp_bb_run_done:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_run_done_return_done
#                         block _dp_bb_run_done_return_done:
#                             raise StopIteration()
#                     block _dp_bb_run_start:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         jump _dp_bb_run_69
#             else:
#                 block _dp_bb_run_dispatch_throw_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_run_dispatch_throw_done, _dp_bb_run_dispatch_throw_unstarted, _dp_bb_run_9, _dp_bb_run_33, _dp_bb_run_56, _dp_bb_run_71] default _dp_bb_run_invalid
#                     block _dp_bb_run_dispatch_throw_done:
#                         raise _dp_resume_exc
#                     block _dp_bb_run_dispatch_throw_unstarted:
#                         raise _dp_resume_exc
#         block _dp_bb_run_69:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
#             __dp_store_cell(_dp_cell__dp_with_exit_1, _dp_with_exit_1)
#             jump _dp_bb_run_49
#             block _dp_bb_run_49:
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                 _dp_yield_from_iter_71 = iter(__dp_await_iter(__dp_asynccontextmanager_aenter(cm)))
#                 __dp_store_cell(_dp_cell__dp_yield_from_iter_71, _dp_yield_from_iter_71)
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_71)
#                 try_jump:
#                     body_label: _dp_bb_run_50
#                     except_label: _dp_bb_run_51
#         block _dp_bb_run_50:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_yield_from_y_72 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_72, _dp_yield_from_y_72)
#             jump _dp_bb_run_55
#         block _dp_bb_run_55:
#             _dp_yield_from_y_72 = __dp_load_deleted_name("_dp_yield_from_y_72", __dp_load_cell(_dp_cell__dp_yield_from_y_72))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_72, _dp_yield_from_y_72)
#             __dp_store_cell(_dp_cell__dp_pc, 4)
#             return _dp_yield_from_y_72
#         block _dp_bb_run_51:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_75 = __dp_load_deleted_name("_dp_try_exc_75", __dp_load_cell(_dp_cell__dp_try_exc_75))
#             __dp_store_cell(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             _dp_try_exc_75 = __dp_current_exception()
#             __dp_store_cell(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             if_term __dp_exception_matches(_dp_try_exc_75, StopIteration):
#                 then:
#                     block _dp_bb_run_52:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_75 = __dp_load_deleted_name("_dp_try_exc_75", __dp_load_cell(_dp_cell__dp_try_exc_75))
#                         __dp_store_cell(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         _dp_yield_from_result_74 = _dp_try_exc_75.value
#                         __dp_store_cell(_dp_cell__dp_yield_from_result_74, _dp_yield_from_result_74)
#                         jump _dp_bb_run_53
#                         block _dp_bb_run_53:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                             jump _dp_bb_run_68
#                             block _dp_bb_run_68:
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                 _dp_yield_from_result_74 = __dp_load_deleted_name("_dp_yield_from_result_74", __dp_load_cell(_dp_cell__dp_yield_from_result_74))
#                                 __dp_store_cell(_dp_cell__dp_yield_from_result_74, _dp_yield_from_result_74)
#                                 x = _dp_yield_from_result_74
#                                 __dp_store_cell(_dp_cell_x, x)
#                                 jump _dp_bb_run_48
#                                 block _dp_bb_run_48:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                     _dp_with_ok_2 = True
#                                     __dp_store_cell(_dp_cell__dp_with_ok_2, _dp_with_ok_2)
#                                     try_jump:
#                                         body_label: _dp_bb_run_47
#                                         except_label: _dp_bb_run_46
#                                     block _dp_bb_run_47:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                         body()
#                                         jump _dp_bb_run_23
#                                     block _dp_bb_run_46:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                         _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                         __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                         _dp_with_ok_2 = False
#                                         __dp_store_cell(_dp_cell__dp_with_ok_2, _dp_with_ok_2)
#                                         jump _dp_bb_run_26
#                                         block _dp_bb_run_26:
#                                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                             _dp_with_exit_1 = __dp_load_deleted_name("_dp_with_exit_1", __dp_load_cell(_dp_cell__dp_with_exit_1))
#                                             __dp_store_cell(_dp_cell__dp_with_exit_1, _dp_with_exit_1)
#                                             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                             _dp_yield_from_iter_40 = iter(__dp_await_iter(__dp_asynccontextmanager_exit(_dp_with_exit_1, __dp_exc_info())))
#                                             __dp_store_cell(_dp_cell__dp_yield_from_iter_40, _dp_yield_from_iter_40)
#                                             __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_40)
#                                             try_jump:
#                                                 body_label: _dp_bb_run_27
#                                                 except_label: _dp_bb_run_28
#                 else:
#                     block _dp_bb_run_54:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         _dp_try_exc_75 = __dp_load_deleted_name("_dp_try_exc_75", __dp_load_cell(_dp_cell__dp_try_exc_75))
#                         __dp_store_cell(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         _dp_yield_from_raise_77 = _dp_try_exc_75
#                         __dp_store_cell(_dp_cell__dp_yield_from_raise_77, _dp_yield_from_raise_77)
#                         jump _dp_bb_run_61
#         block _dp_bb_run_23:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_3 = None
#             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             jump _dp_bb_run_22
#             block _dp_bb_run_22:
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                 _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                 __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                 _dp_with_ok_2 = __dp_load_deleted_name("_dp_with_ok_2", __dp_load_cell(_dp_cell__dp_with_ok_2))
#                 __dp_store_cell(_dp_cell__dp_with_ok_2, _dp_with_ok_2)
#                 _dp_try_exc_3 = __dp_current_exception()
#                 __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                 if_term _dp_with_ok_2:
#                     then:
#                         block _dp_bb_run_21:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             jump _dp_bb_run_2
#                             block _dp_bb_run_2:
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                 _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                 __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                 _dp_with_exit_1 = __dp_load_deleted_name("_dp_with_exit_1", __dp_load_cell(_dp_cell__dp_with_exit_1))
#                                 __dp_store_cell(_dp_cell__dp_with_exit_1, _dp_with_exit_1)
#                                 _dp_yield_from_iter_9 = iter(__dp_await_iter(__dp_asynccontextmanager_exit(_dp_with_exit_1, None)))
#                                 __dp_store_cell(_dp_cell__dp_yield_from_iter_9, _dp_yield_from_iter_9)
#                                 __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_9)
#                                 try_jump:
#                                     body_label: _dp_bb_run_3
#                                     except_label: _dp_bb_run_4
#                     else:
#                         jump _dp_bb_run_1
#         block _dp_bb_run_3:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_yield_from_y_10 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_10, _dp_yield_from_y_10)
#             jump _dp_bb_run_8
#         block _dp_bb_run_8:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_yield_from_y_10 = __dp_load_deleted_name("_dp_yield_from_y_10", __dp_load_cell(_dp_cell__dp_yield_from_y_10))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_10, _dp_yield_from_y_10)
#             __dp_store_cell(_dp_cell__dp_pc, 2)
#             return _dp_yield_from_y_10
#         block _dp_bb_run_4:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_12 = __dp_load_deleted_name("_dp_try_exc_12", __dp_load_cell(_dp_cell__dp_try_exc_12))
#             __dp_store_cell(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_12 = __dp_current_exception()
#             __dp_store_cell(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             if_term __dp_exception_matches(_dp_try_exc_12, StopIteration):
#                 then:
#                     block _dp_bb_run_5:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                         __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         jump _dp_bb_run_6
#                         block _dp_bb_run_6:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                             jump _dp_bb_run_1
#                 else:
#                     block _dp_bb_run_7:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                         __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         _dp_try_exc_12 = __dp_load_deleted_name("_dp_try_exc_12", __dp_load_cell(_dp_cell__dp_try_exc_12))
#                         __dp_store_cell(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_yield_from_raise_14 = _dp_try_exc_12
#                         __dp_store_cell(_dp_cell__dp_yield_from_raise_14, _dp_yield_from_raise_14)
#                         jump _dp_bb_run_14
#         block _dp_bb_run_1:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_with_exit_1 = None
#             __dp_store_cell(_dp_cell__dp_with_exit_1, _dp_with_exit_1)
#             if_term __dp_is_not(_dp_try_exc_3, None):
#                 then:
#                     block _dp_bb_run_0:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                         __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         raise _dp_try_exc_3
#                 else:
#                     block _dp_bb_run_72:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_run_72_return_done
#                         block _dp_bb_run_72_return_done:
#                             raise StopIteration()
#         block _dp_bb_run_14:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_yield_from_raise_14 = __dp_load_deleted_name("_dp_yield_from_raise_14", __dp_load_cell(_dp_cell__dp_yield_from_raise_14))
#             __dp_store_cell(_dp_cell__dp_yield_from_raise_14, _dp_yield_from_raise_14)
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_14
#         block _dp_bb_run_27:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             _dp_yield_from_y_41 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_41, _dp_yield_from_y_41)
#             jump _dp_bb_run_32
#         block _dp_bb_run_32:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             _dp_yield_from_y_41 = __dp_load_deleted_name("_dp_yield_from_y_41", __dp_load_cell(_dp_cell__dp_yield_from_y_41))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_41, _dp_yield_from_y_41)
#             __dp_store_cell(_dp_cell__dp_pc, 3)
#             return _dp_yield_from_y_41
#         block _dp_bb_run_28:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_44 = __dp_load_deleted_name("_dp_try_exc_44", __dp_load_cell(_dp_cell__dp_try_exc_44))
#             __dp_store_cell(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             _dp_try_exc_44 = __dp_current_exception()
#             __dp_store_cell(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             if_term __dp_exception_matches(_dp_try_exc_44, StopIteration):
#                 then:
#                     block _dp_bb_run_29:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                         __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         _dp_try_exc_44 = __dp_load_deleted_name("_dp_try_exc_44", __dp_load_cell(_dp_cell__dp_try_exc_44))
#                         __dp_store_cell(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         _dp_yield_from_result_43 = _dp_try_exc_44.value
#                         __dp_store_cell(_dp_cell__dp_yield_from_result_43, _dp_yield_from_result_43)
#                         jump _dp_bb_run_30
#                         block _dp_bb_run_30:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                             jump _dp_bb_run_45
#                             block _dp_bb_run_45:
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                 _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                 __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                 _dp_yield_from_result_43 = __dp_load_deleted_name("_dp_yield_from_result_43", __dp_load_cell(_dp_cell__dp_yield_from_result_43))
#                                 __dp_store_cell(_dp_cell__dp_yield_from_result_43, _dp_yield_from_result_43)
#                                 _dp_with_reraise_3 = _dp_yield_from_result_43
#                                 __dp_store_cell(_dp_cell__dp_with_reraise_3, _dp_with_reraise_3)
#                                 jump _dp_bb_run_25
#                                 block _dp_bb_run_25:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                     _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                     __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     _dp_with_reraise_3 = __dp_load_deleted_name("_dp_with_reraise_3", __dp_load_cell(_dp_cell__dp_with_reraise_3))
#                                     __dp_store_cell(_dp_cell__dp_with_reraise_3, _dp_with_reraise_3)
#                                     if_term __dp_is_not(_dp_with_reraise_3, None):
#                                         then:
#                                             block _dp_bb_run_24:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                 _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                                 __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 _dp_with_reraise_3 = __dp_load_deleted_name("_dp_with_reraise_3", __dp_load_cell(_dp_cell__dp_with_reraise_3))
#                                                 __dp_store_cell(_dp_cell__dp_with_reraise_3, _dp_with_reraise_3)
#                                                 raise _dp_with_reraise_3
#                                         else:
#                                             jump _dp_bb_run_23
#                 else:
#                     block _dp_bb_run_31:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                         __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         _dp_try_exc_44 = __dp_load_deleted_name("_dp_try_exc_44", __dp_load_cell(_dp_cell__dp_try_exc_44))
#                         __dp_store_cell(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         _dp_yield_from_raise_46 = _dp_try_exc_44
#                         __dp_store_cell(_dp_cell__dp_yield_from_raise_46, _dp_yield_from_raise_46)
#                         jump _dp_bb_run_38
#         block _dp_bb_run_38:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             _dp_yield_from_raise_46 = __dp_load_deleted_name("_dp_yield_from_raise_46", __dp_load_cell(_dp_cell__dp_yield_from_raise_46))
#             __dp_store_cell(_dp_cell__dp_yield_from_raise_46, _dp_yield_from_raise_46)
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_46
#         block _dp_bb_run_61:
#             _dp_yield_from_raise_77 = __dp_load_deleted_name("_dp_yield_from_raise_77", __dp_load_cell(_dp_cell__dp_yield_from_raise_77))
#             __dp_store_cell(_dp_cell__dp_yield_from_raise_77, _dp_yield_from_raise_77)
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_77
#         block _dp_bb_run_9:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             _dp_yield_from_sent_11 = _dp_send_value
#             __dp_store_cell(_dp_cell__dp_yield_from_sent_11, _dp_yield_from_sent_11)
#             _dp_yield_from_exc_13 = _dp_resume_exc
#             __dp_store_cell(_dp_cell__dp_yield_from_exc_13, _dp_yield_from_exc_13)
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_13, None):
#                 then:
#                     block _dp_bb_run_10:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                         __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         _dp_yield_from_exc_13 = __dp_load_deleted_name("_dp_yield_from_exc_13", __dp_load_cell(_dp_cell__dp_yield_from_exc_13))
#                         __dp_store_cell(_dp_cell__dp_yield_from_exc_13, _dp_yield_from_exc_13)
#                         if_term __dp_exception_matches(_dp_yield_from_exc_13, GeneratorExit):
#                             then:
#                                 block _dp_bb_run_11:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                     __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     _dp_yield_from_close_15 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_close_15, _dp_yield_from_close_15)
#                                     if_term __dp_is_not(_dp_yield_from_close_15, None):
#                                         then:
#                                             block _dp_bb_run_12:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                                 __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 _dp_yield_from_close_15 = __dp_load_deleted_name("_dp_yield_from_close_15", __dp_load_cell(_dp_cell__dp_yield_from_close_15))
#                                                 __dp_store_cell(_dp_cell__dp_yield_from_close_15, _dp_yield_from_close_15)
#                                                 _dp_yield_from_close_15()
#                                                 jump _dp_bb_run_13
#                                         else:
#                                             jump _dp_bb_run_13
#                             else:
#                                 block _dp_bb_run_15:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                     _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                     __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     _dp_yield_from_throw_16 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_16, _dp_yield_from_throw_16)
#                                     if_term __dp_is_(_dp_yield_from_throw_16, None):
#                                         then:
#                                             jump _dp_bb_run_13
#                                         else:
#                                             block _dp_bb_run_16:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                 _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                                 __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 try_jump:
#                                                     body_label: _dp_bb_run_17
#                                                     except_label: _dp_bb_run_4
#                                                 block _dp_bb_run_17:
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                     _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                                     __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                     _dp_yield_from_exc_13 = __dp_load_deleted_name("_dp_yield_from_exc_13", __dp_load_cell(_dp_cell__dp_yield_from_exc_13))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_exc_13, _dp_yield_from_exc_13)
#                                                     _dp_yield_from_throw_16 = __dp_load_deleted_name("_dp_yield_from_throw_16", __dp_load_cell(_dp_cell__dp_yield_from_throw_16))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_16, _dp_yield_from_throw_16)
#                                                     _dp_yield_from_y_10 = _dp_yield_from_throw_16(_dp_yield_from_exc_13)
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_y_10, _dp_yield_from_y_10)
#                                                     jump _dp_bb_run_8
#                         block _dp_bb_run_13:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             _dp_yield_from_exc_13 = __dp_load_deleted_name("_dp_yield_from_exc_13", __dp_load_cell(_dp_cell__dp_yield_from_exc_13))
#                             __dp_store_cell(_dp_cell__dp_yield_from_exc_13, _dp_yield_from_exc_13)
#                             _dp_yield_from_raise_14 = _dp_yield_from_exc_13
#                             __dp_store_cell(_dp_cell__dp_yield_from_raise_14, _dp_yield_from_raise_14)
#                             jump _dp_bb_run_14
#                 else:
#                     block _dp_bb_run_18:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                         __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         try_jump:
#                             body_label: _dp_bb_run_19
#                             except_label: _dp_bb_run_4
#                         block _dp_bb_run_19:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                             __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             _dp_yield_from_sent_11 = __dp_load_deleted_name("_dp_yield_from_sent_11", __dp_load_cell(_dp_cell__dp_yield_from_sent_11))
#                             __dp_store_cell(_dp_cell__dp_yield_from_sent_11, _dp_yield_from_sent_11)
#                             if_term __dp_is_(_dp_yield_from_sent_11, None):
#                                 then:
#                                     jump _dp_bb_run_3
#                                 else:
#                                     block _dp_bb_run_20:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                         _dp_try_exc_3 = __dp_load_deleted_name("_dp_try_exc_3", __dp_load_cell(_dp_cell__dp_try_exc_3))
#                                         __dp_store_cell(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                         _dp_yield_from_sent_11 = __dp_load_deleted_name("_dp_yield_from_sent_11", __dp_load_cell(_dp_cell__dp_yield_from_sent_11))
#                                         __dp_store_cell(_dp_cell__dp_yield_from_sent_11, _dp_yield_from_sent_11)
#                                         _dp_yield_from_y_10 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_11)
#                                         __dp_store_cell(_dp_cell__dp_yield_from_y_10, _dp_yield_from_y_10)
#                                         jump _dp_bb_run_8
#         block _dp_bb_run_33:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             _dp_yield_from_sent_42 = _dp_send_value
#             __dp_store_cell(_dp_cell__dp_yield_from_sent_42, _dp_yield_from_sent_42)
#             _dp_yield_from_exc_45 = _dp_resume_exc
#             __dp_store_cell(_dp_cell__dp_yield_from_exc_45, _dp_yield_from_exc_45)
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_45, None):
#                 then:
#                     block _dp_bb_run_34:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                         __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         _dp_yield_from_exc_45 = __dp_load_deleted_name("_dp_yield_from_exc_45", __dp_load_cell(_dp_cell__dp_yield_from_exc_45))
#                         __dp_store_cell(_dp_cell__dp_yield_from_exc_45, _dp_yield_from_exc_45)
#                         if_term __dp_exception_matches(_dp_yield_from_exc_45, GeneratorExit):
#                             then:
#                                 block _dp_bb_run_35:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                     _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                     __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     _dp_yield_from_close_47 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_close_47, _dp_yield_from_close_47)
#                                     if_term __dp_is_not(_dp_yield_from_close_47, None):
#                                         then:
#                                             block _dp_bb_run_36:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                 _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                                 __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 _dp_yield_from_close_47 = __dp_load_deleted_name("_dp_yield_from_close_47", __dp_load_cell(_dp_cell__dp_yield_from_close_47))
#                                                 __dp_store_cell(_dp_cell__dp_yield_from_close_47, _dp_yield_from_close_47)
#                                                 _dp_yield_from_close_47()
#                                                 jump _dp_bb_run_37
#                                         else:
#                                             jump _dp_bb_run_37
#                             else:
#                                 block _dp_bb_run_39:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                     _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                     __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     _dp_yield_from_throw_48 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_48, _dp_yield_from_throw_48)
#                                     if_term __dp_is_(_dp_yield_from_throw_48, None):
#                                         then:
#                                             jump _dp_bb_run_37
#                                         else:
#                                             block _dp_bb_run_40:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                 _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                                 __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 try_jump:
#                                                     body_label: _dp_bb_run_41
#                                                     except_label: _dp_bb_run_28
#                                                 block _dp_bb_run_41:
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                     _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                                     __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                     _dp_yield_from_exc_45 = __dp_load_deleted_name("_dp_yield_from_exc_45", __dp_load_cell(_dp_cell__dp_yield_from_exc_45))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_exc_45, _dp_yield_from_exc_45)
#                                                     _dp_yield_from_throw_48 = __dp_load_deleted_name("_dp_yield_from_throw_48", __dp_load_cell(_dp_cell__dp_yield_from_throw_48))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_48, _dp_yield_from_throw_48)
#                                                     _dp_yield_from_y_41 = _dp_yield_from_throw_48(_dp_yield_from_exc_45)
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_y_41, _dp_yield_from_y_41)
#                                                     jump _dp_bb_run_32
#                         block _dp_bb_run_37:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             _dp_yield_from_exc_45 = __dp_load_deleted_name("_dp_yield_from_exc_45", __dp_load_cell(_dp_cell__dp_yield_from_exc_45))
#                             __dp_store_cell(_dp_cell__dp_yield_from_exc_45, _dp_yield_from_exc_45)
#                             _dp_yield_from_raise_46 = _dp_yield_from_exc_45
#                             __dp_store_cell(_dp_cell__dp_yield_from_raise_46, _dp_yield_from_raise_46)
#                             jump _dp_bb_run_38
#                 else:
#                     block _dp_bb_run_42:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                         __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         try_jump:
#                             body_label: _dp_bb_run_43
#                             except_label: _dp_bb_run_28
#                         block _dp_bb_run_43:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                             __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             _dp_yield_from_sent_42 = __dp_load_deleted_name("_dp_yield_from_sent_42", __dp_load_cell(_dp_cell__dp_yield_from_sent_42))
#                             __dp_store_cell(_dp_cell__dp_yield_from_sent_42, _dp_yield_from_sent_42)
#                             if_term __dp_is_(_dp_yield_from_sent_42, None):
#                                 then:
#                                     jump _dp_bb_run_27
#                                 else:
#                                     block _dp_bb_run_44:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                         _dp_try_exc_2 = __dp_load_deleted_name("_dp_try_exc_2", __dp_load_cell(_dp_cell__dp_try_exc_2))
#                                         __dp_store_cell(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                         _dp_yield_from_sent_42 = __dp_load_deleted_name("_dp_yield_from_sent_42", __dp_load_cell(_dp_cell__dp_yield_from_sent_42))
#                                         __dp_store_cell(_dp_cell__dp_yield_from_sent_42, _dp_yield_from_sent_42)
#                                         _dp_yield_from_y_41 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_42)
#                                         __dp_store_cell(_dp_cell__dp_yield_from_y_41, _dp_yield_from_y_41)
#                                         jump _dp_bb_run_32
#         block _dp_bb_run_56:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             _dp_yield_from_sent_73 = _dp_send_value
#             __dp_store_cell(_dp_cell__dp_yield_from_sent_73, _dp_yield_from_sent_73)
#             _dp_yield_from_exc_76 = _dp_resume_exc
#             __dp_store_cell(_dp_cell__dp_yield_from_exc_76, _dp_yield_from_exc_76)
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_76, None):
#                 then:
#                     block _dp_bb_run_57:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         _dp_yield_from_exc_76 = __dp_load_deleted_name("_dp_yield_from_exc_76", __dp_load_cell(_dp_cell__dp_yield_from_exc_76))
#                         __dp_store_cell(_dp_cell__dp_yield_from_exc_76, _dp_yield_from_exc_76)
#                         if_term __dp_exception_matches(_dp_yield_from_exc_76, GeneratorExit):
#                             then:
#                                 block _dp_bb_run_58:
#                                     _dp_yield_from_close_78 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_close_78, _dp_yield_from_close_78)
#                                     if_term __dp_is_not(_dp_yield_from_close_78, None):
#                                         then:
#                                             block _dp_bb_run_59:
#                                                 _dp_yield_from_close_78 = __dp_load_deleted_name("_dp_yield_from_close_78", __dp_load_cell(_dp_cell__dp_yield_from_close_78))
#                                                 __dp_store_cell(_dp_cell__dp_yield_from_close_78, _dp_yield_from_close_78)
#                                                 _dp_yield_from_close_78()
#                                                 jump _dp_bb_run_60
#                                         else:
#                                             jump _dp_bb_run_60
#                             else:
#                                 block _dp_bb_run_62:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                     _dp_yield_from_throw_79 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_79, _dp_yield_from_throw_79)
#                                     if_term __dp_is_(_dp_yield_from_throw_79, None):
#                                         then:
#                                             jump _dp_bb_run_60
#                                         else:
#                                             block _dp_bb_run_63:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                 try_jump:
#                                                     body_label: _dp_bb_run_64
#                                                     except_label: _dp_bb_run_51
#                                                 block _dp_bb_run_64:
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                                     _dp_yield_from_exc_76 = __dp_load_deleted_name("_dp_yield_from_exc_76", __dp_load_cell(_dp_cell__dp_yield_from_exc_76))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_exc_76, _dp_yield_from_exc_76)
#                                                     _dp_yield_from_throw_79 = __dp_load_deleted_name("_dp_yield_from_throw_79", __dp_load_cell(_dp_cell__dp_yield_from_throw_79))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_79, _dp_yield_from_throw_79)
#                                                     _dp_yield_from_y_72 = _dp_yield_from_throw_79(_dp_yield_from_exc_76)
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_y_72, _dp_yield_from_y_72)
#                                                     jump _dp_bb_run_55
#                         block _dp_bb_run_60:
#                             _dp_yield_from_exc_76 = __dp_load_deleted_name("_dp_yield_from_exc_76", __dp_load_cell(_dp_cell__dp_yield_from_exc_76))
#                             __dp_store_cell(_dp_cell__dp_yield_from_exc_76, _dp_yield_from_exc_76)
#                             _dp_yield_from_raise_77 = _dp_yield_from_exc_76
#                             __dp_store_cell(_dp_cell__dp_yield_from_raise_77, _dp_yield_from_raise_77)
#                             jump _dp_bb_run_61
#                 else:
#                     block _dp_bb_run_65:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                         try_jump:
#                             body_label: _dp_bb_run_66
#                             except_label: _dp_bb_run_51
#                         block _dp_bb_run_66:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                             _dp_yield_from_sent_73 = __dp_load_deleted_name("_dp_yield_from_sent_73", __dp_load_cell(_dp_cell__dp_yield_from_sent_73))
#                             __dp_store_cell(_dp_cell__dp_yield_from_sent_73, _dp_yield_from_sent_73)
#                             if_term __dp_is_(_dp_yield_from_sent_73, None):
#                                 then:
#                                     jump _dp_bb_run_50
#                                 else:
#                                     block _dp_bb_run_67:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#                                         _dp_yield_from_sent_73 = __dp_load_deleted_name("_dp_yield_from_sent_73", __dp_load_cell(_dp_cell__dp_yield_from_sent_73))
#                                         __dp_store_cell(_dp_cell__dp_yield_from_sent_73, _dp_yield_from_sent_73)
#                                         _dp_yield_from_y_72 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_73)
#                                         __dp_store_cell(_dp_cell__dp_yield_from_y_72, _dp_yield_from_y_72)
#                                         jump _dp_bb_run_55
#         block _dp_bb_run_71:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_44, _dp_try_exc_44)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_2, _dp_try_exc_2)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_75, _dp_try_exc_75)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_3, _dp_try_exc_3)
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_12, _dp_try_exc_12)
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_run_70:
#                         raise _dp_resume_exc
#                 else:
#                     jump _dp_bb_run_69
#         block _dp_bb_run_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     block _dp_bb_run_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 jump _dp_bb_run_uncaught_set_done
#             else:
#                 jump _dp_bb_run_uncaught_raise
#     block _dp_bb_run_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_store_cell(_dp_cell__dp_try_exc_3, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_with_exit_1, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_iter_9, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_y_10, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_12, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_raise_14, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_exc_13, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_sent_11, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_close_15, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_throw_16, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_with_ok_2, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_2, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_with_reraise_3, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_iter_40, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_y_41, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_44, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_result_43, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_raise_46, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_exc_45, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_sent_42, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_close_47, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_throw_48, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_iter_71, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_y_72, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_75, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_result_74, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_raise_77, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_exc_76, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_sent_73, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_close_78, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_throw_79, __dp_DELETED)
#         __dp_store_cell(_dp_cell_x, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, __dp_DELETED)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_102)
#         jump _dp_bb_run_uncaught_raise
#     block _dp_bb_run_uncaught_raise:
#         raise _dp_uncaught_exc_102

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "run", __dp_mark_coroutine_function(__dp_make_function("start", 0, "run", "run", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None)))
#         return

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         _dp_match_1 = value
#         if_term __dp_eq(_dp_match_1, 1):
#             then:
#                 block _dp_bb__dp_module_init_0:
#                     one()
#                     return
#             else:
#                 block _dp_bb__dp_module_init_1:
#                     other()
#                     return

# generator_yield


def gen():
    yield 1


# ==

# module_init: _dp_module_init

# function gen()
#     kind: function
#     bind: gen
#     qualname: gen
#     local_cell_slots: [_dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         _dp_cell__dp_pc = __dp_make_cell(1)
#         _dp_cell__dp_yieldfrom = __dp_make_cell(None)
#         return __dp_make_closure_generator(__dp_def_hidden_resume_fn("start", 1, "_dp_resume", "gen", __dp_tuple("_dp_self", "_dp_send_value", "_dp_resume_exc", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple("_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple(_dp_cell__dp_pc, _dp_cell__dp_yieldfrom), __dp_globals(), __name__, async_gen=False), "gen", "gen")

# function gen_resume()
#     kind: generator
#     bind: gen_resume
#     qualname: gen
#     display_name: _dp_resume
#     entry_liveins: [_dp_self, _dp_send_value, _dp_resume_exc, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     local_cell_slots: [_dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block _dp_bb_gen_dispatch_send_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_gen_done, _dp_bb_gen_start, _dp_bb_gen_1] default _dp_bb_gen_invalid
#                     block _dp_bb_gen_done:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_gen_done_return_done
#                         block _dp_bb_gen_done_return_done:
#                             raise StopIteration()
#                     block _dp_bb_gen_start:
#                         __dp_store_cell(_dp_cell__dp_pc, 2)
#                         return 1
#             else:
#                 block _dp_bb_gen_dispatch_throw_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_gen_dispatch_throw_done, _dp_bb_gen_dispatch_throw_unstarted, _dp_bb_gen_1] default _dp_bb_gen_invalid
#                     block _dp_bb_gen_dispatch_throw_done:
#                         raise _dp_resume_exc
#                     block _dp_bb_gen_dispatch_throw_unstarted:
#                         raise _dp_resume_exc
#         block _dp_bb_gen_1:
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_gen_0:
#                         raise _dp_resume_exc
#                 else:
#                     block _dp_bb_gen_2:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_gen_2_return_done
#                         block _dp_bb_gen_2_return_done:
#                             raise StopIteration()
#         block _dp_bb_gen_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     block _dp_bb_gen_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 jump _dp_bb_gen_uncaught_set_done
#             else:
#                 jump _dp_bb_gen_uncaught_raise
#     block _dp_bb_gen_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, __dp_DELETED)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_4)
#         jump _dp_bb_gen_uncaught_raise
#     block _dp_bb_gen_uncaught_raise:
#         raise _dp_uncaught_exc_4

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "gen", __dp_make_function("start", 0, "gen", "gen", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# yield_from


def gen():
    yield from it


# ==

# module_init: _dp_module_init

# function gen()
#     kind: function
#     bind: gen
#     qualname: gen
#     local_cell_slots: [_dp_cell__dp_pc, _dp_cell__dp_try_exc_5, _dp_cell__dp_yield_from_close_8, _dp_cell__dp_yield_from_exc_6, _dp_cell__dp_yield_from_iter_2, _dp_cell__dp_yield_from_raise_7, _dp_cell__dp_yield_from_sent_4, _dp_cell__dp_yield_from_throw_9, _dp_cell__dp_yield_from_y_3, _dp_cell__dp_yieldfrom]
#     cellvars: [_dp_yield_from_iter_2->_dp_cell__dp_yield_from_iter_2@deferred, _dp_yield_from_y_3->_dp_cell__dp_yield_from_y_3@deferred, _dp_try_exc_5->_dp_cell__dp_try_exc_5@deleted, _dp_yield_from_raise_7->_dp_cell__dp_yield_from_raise_7@deferred, _dp_yield_from_exc_6->_dp_cell__dp_yield_from_exc_6@deferred, _dp_yield_from_sent_4->_dp_cell__dp_yield_from_sent_4@deferred, _dp_yield_from_close_8->_dp_cell__dp_yield_from_close_8@deferred, _dp_yield_from_throw_9->_dp_cell__dp_yield_from_throw_9@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         _dp_cell__dp_yield_from_iter_2 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_y_3 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_5 = __dp_make_cell(__dp_DELETED)
#         _dp_cell__dp_yield_from_raise_7 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_exc_6 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_sent_4 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_close_8 = __dp_make_cell(None)
#         _dp_cell__dp_yield_from_throw_9 = __dp_make_cell(None)
#         _dp_cell__dp_pc = __dp_make_cell(1)
#         _dp_cell__dp_yieldfrom = __dp_make_cell(None)
#         return __dp_make_closure_generator(__dp_def_hidden_resume_fn("start", 1, "_dp_resume", "gen", __dp_tuple("_dp_self", "_dp_send_value", "_dp_resume_exc", "_dp_cell__dp_yield_from_iter_2", "_dp_cell__dp_yield_from_y_3", "_dp_cell__dp_try_exc_5", "_dp_cell__dp_yield_from_raise_7", "_dp_cell__dp_yield_from_exc_6", "_dp_cell__dp_yield_from_sent_4", "_dp_cell__dp_yield_from_close_8", "_dp_cell__dp_yield_from_throw_9", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple("_dp_cell__dp_yield_from_iter_2", "_dp_cell__dp_yield_from_y_3", "_dp_cell__dp_try_exc_5", "_dp_cell__dp_yield_from_raise_7", "_dp_cell__dp_yield_from_exc_6", "_dp_cell__dp_yield_from_sent_4", "_dp_cell__dp_yield_from_close_8", "_dp_cell__dp_yield_from_throw_9", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple(_dp_cell__dp_yield_from_iter_2, _dp_cell__dp_yield_from_y_3, _dp_cell__dp_try_exc_5, _dp_cell__dp_yield_from_raise_7, _dp_cell__dp_yield_from_exc_6, _dp_cell__dp_yield_from_sent_4, _dp_cell__dp_yield_from_close_8, _dp_cell__dp_yield_from_throw_9, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom), __dp_globals(), __name__, async_gen=False), "gen", "gen")

# function gen_resume()
#     kind: generator
#     bind: gen_resume
#     qualname: gen
#     display_name: _dp_resume
#     entry_liveins: [_dp_self, _dp_send_value, _dp_resume_exc, _dp_cell__dp_yield_from_iter_2, _dp_cell__dp_yield_from_y_3, _dp_cell__dp_try_exc_5, _dp_cell__dp_yield_from_raise_7, _dp_cell__dp_yield_from_exc_6, _dp_cell__dp_yield_from_sent_4, _dp_cell__dp_yield_from_close_8, _dp_cell__dp_yield_from_throw_9, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     local_cell_slots: [_dp_cell__dp_pc, _dp_cell__dp_try_exc_5, _dp_cell__dp_yield_from_close_8, _dp_cell__dp_yield_from_exc_6, _dp_cell__dp_yield_from_iter_2, _dp_cell__dp_yield_from_raise_7, _dp_cell__dp_yield_from_sent_4, _dp_cell__dp_yield_from_throw_9, _dp_cell__dp_yield_from_y_3, _dp_cell__dp_yieldfrom]
#     cellvars: [_dp_yield_from_iter_2->_dp_cell__dp_yield_from_iter_2@deferred, _dp_yield_from_y_3->_dp_cell__dp_yield_from_y_3@deferred, _dp_try_exc_5->_dp_cell__dp_try_exc_5@deleted, _dp_yield_from_raise_7->_dp_cell__dp_yield_from_raise_7@deferred, _dp_yield_from_exc_6->_dp_cell__dp_yield_from_exc_6@deferred, _dp_yield_from_sent_4->_dp_cell__dp_yield_from_sent_4@deferred, _dp_yield_from_close_8->_dp_cell__dp_yield_from_close_8@deferred, _dp_yield_from_throw_9->_dp_cell__dp_yield_from_throw_9@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block _dp_bb_gen_dispatch_send_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_gen_done, _dp_bb_gen_start, _dp_bb_gen_7] default _dp_bb_gen_invalid
#                     block _dp_bb_gen_done:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_gen_done_return_done
#                         block _dp_bb_gen_done_return_done:
#                             raise StopIteration()
#                     block _dp_bb_gen_start:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                         jump _dp_bb_gen_0
#                         block _dp_bb_gen_0:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                             _dp_yield_from_iter_2 = iter(it)
#                             __dp_store_cell(_dp_cell__dp_yield_from_iter_2, _dp_yield_from_iter_2)
#                             __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_2)
#                             try_jump:
#                                 body_label: _dp_bb_gen_1
#                                 except_label: _dp_bb_gen_2
#             else:
#                 block _dp_bb_gen_dispatch_throw_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_gen_dispatch_throw_done, _dp_bb_gen_dispatch_throw_unstarted, _dp_bb_gen_7] default _dp_bb_gen_invalid
#                     block _dp_bb_gen_dispatch_throw_done:
#                         raise _dp_resume_exc
#                     block _dp_bb_gen_dispatch_throw_unstarted:
#                         raise _dp_resume_exc
#         block _dp_bb_gen_1:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#             _dp_yield_from_y_3 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_3, _dp_yield_from_y_3)
#             jump _dp_bb_gen_6
#         block _dp_bb_gen_6:
#             _dp_yield_from_y_3 = __dp_load_deleted_name("_dp_yield_from_y_3", __dp_load_cell(_dp_cell__dp_yield_from_y_3))
#             __dp_store_cell(_dp_cell__dp_yield_from_y_3, _dp_yield_from_y_3)
#             __dp_store_cell(_dp_cell__dp_pc, 2)
#             return _dp_yield_from_y_3
#         block _dp_bb_gen_2:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#             _dp_try_exc_5 = __dp_load_deleted_name("_dp_try_exc_5", __dp_load_cell(_dp_cell__dp_try_exc_5))
#             __dp_store_cell(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#             _dp_try_exc_5 = __dp_current_exception()
#             __dp_store_cell(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#             if_term __dp_exception_matches(_dp_try_exc_5, StopIteration):
#                 then:
#                     block _dp_bb_gen_3:
#                         jump _dp_bb_gen_4
#                         block _dp_bb_gen_4:
#                             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                             __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                             jump _dp_bb_gen_4_return_done
#                             block _dp_bb_gen_4_return_done:
#                                 raise StopIteration()
#                 else:
#                     block _dp_bb_gen_5:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                         _dp_try_exc_5 = __dp_load_deleted_name("_dp_try_exc_5", __dp_load_cell(_dp_cell__dp_try_exc_5))
#                         __dp_store_cell(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                         _dp_yield_from_raise_7 = _dp_try_exc_5
#                         __dp_store_cell(_dp_cell__dp_yield_from_raise_7, _dp_yield_from_raise_7)
#                         jump _dp_bb_gen_12
#         block _dp_bb_gen_12:
#             _dp_yield_from_raise_7 = __dp_load_deleted_name("_dp_yield_from_raise_7", __dp_load_cell(_dp_cell__dp_yield_from_raise_7))
#             __dp_store_cell(_dp_cell__dp_yield_from_raise_7, _dp_yield_from_raise_7)
#             __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#             raise _dp_yield_from_raise_7
#         block _dp_bb_gen_7:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#             _dp_yield_from_sent_4 = _dp_send_value
#             __dp_store_cell(_dp_cell__dp_yield_from_sent_4, _dp_yield_from_sent_4)
#             _dp_yield_from_exc_6 = _dp_resume_exc
#             __dp_store_cell(_dp_cell__dp_yield_from_exc_6, _dp_yield_from_exc_6)
#             _dp_resume_exc = None
#             if_term __dp_is_not(_dp_yield_from_exc_6, None):
#                 then:
#                     block _dp_bb_gen_8:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                         _dp_yield_from_exc_6 = __dp_load_deleted_name("_dp_yield_from_exc_6", __dp_load_cell(_dp_cell__dp_yield_from_exc_6))
#                         __dp_store_cell(_dp_cell__dp_yield_from_exc_6, _dp_yield_from_exc_6)
#                         if_term __dp_exception_matches(_dp_yield_from_exc_6, GeneratorExit):
#                             then:
#                                 block _dp_bb_gen_9:
#                                     _dp_yield_from_close_8 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_close_8, _dp_yield_from_close_8)
#                                     if_term __dp_is_not(_dp_yield_from_close_8, None):
#                                         then:
#                                             block _dp_bb_gen_10:
#                                                 _dp_yield_from_close_8 = __dp_load_deleted_name("_dp_yield_from_close_8", __dp_load_cell(_dp_cell__dp_yield_from_close_8))
#                                                 __dp_store_cell(_dp_cell__dp_yield_from_close_8, _dp_yield_from_close_8)
#                                                 _dp_yield_from_close_8()
#                                                 jump _dp_bb_gen_11
#                                         else:
#                                             jump _dp_bb_gen_11
#                             else:
#                                 block _dp_bb_gen_13:
#                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                                     _dp_yield_from_throw_9 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_9, _dp_yield_from_throw_9)
#                                     if_term __dp_is_(_dp_yield_from_throw_9, None):
#                                         then:
#                                             jump _dp_bb_gen_11
#                                         else:
#                                             block _dp_bb_gen_14:
#                                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                                                 try_jump:
#                                                     body_label: _dp_bb_gen_15
#                                                     except_label: _dp_bb_gen_2
#                                                 block _dp_bb_gen_15:
#                                                     __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                                                     _dp_yield_from_exc_6 = __dp_load_deleted_name("_dp_yield_from_exc_6", __dp_load_cell(_dp_cell__dp_yield_from_exc_6))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_exc_6, _dp_yield_from_exc_6)
#                                                     _dp_yield_from_throw_9 = __dp_load_deleted_name("_dp_yield_from_throw_9", __dp_load_cell(_dp_cell__dp_yield_from_throw_9))
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_throw_9, _dp_yield_from_throw_9)
#                                                     _dp_yield_from_y_3 = _dp_yield_from_throw_9(_dp_yield_from_exc_6)
#                                                     __dp_store_cell(_dp_cell__dp_yield_from_y_3, _dp_yield_from_y_3)
#                                                     jump _dp_bb_gen_6
#                         block _dp_bb_gen_11:
#                             _dp_yield_from_exc_6 = __dp_load_deleted_name("_dp_yield_from_exc_6", __dp_load_cell(_dp_cell__dp_yield_from_exc_6))
#                             __dp_store_cell(_dp_cell__dp_yield_from_exc_6, _dp_yield_from_exc_6)
#                             _dp_yield_from_raise_7 = _dp_yield_from_exc_6
#                             __dp_store_cell(_dp_cell__dp_yield_from_raise_7, _dp_yield_from_raise_7)
#                             jump _dp_bb_gen_12
#                 else:
#                     block _dp_bb_gen_16:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                         try_jump:
#                             body_label: _dp_bb_gen_17
#                             except_label: _dp_bb_gen_2
#                         block _dp_bb_gen_17:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                             _dp_yield_from_sent_4 = __dp_load_deleted_name("_dp_yield_from_sent_4", __dp_load_cell(_dp_cell__dp_yield_from_sent_4))
#                             __dp_store_cell(_dp_cell__dp_yield_from_sent_4, _dp_yield_from_sent_4)
#                             if_term __dp_is_(_dp_yield_from_sent_4, None):
#                                 then:
#                                     jump _dp_bb_gen_1
#                                 else:
#                                     block _dp_bb_gen_18:
#                                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_5, _dp_try_exc_5)
#                                         _dp_yield_from_sent_4 = __dp_load_deleted_name("_dp_yield_from_sent_4", __dp_load_cell(_dp_cell__dp_yield_from_sent_4))
#                                         __dp_store_cell(_dp_cell__dp_yield_from_sent_4, _dp_yield_from_sent_4)
#                                         _dp_yield_from_y_3 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_4)
#                                         __dp_store_cell(_dp_cell__dp_yield_from_y_3, _dp_yield_from_y_3)
#                                         jump _dp_bb_gen_6
#         block _dp_bb_gen_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     block _dp_bb_gen_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 jump _dp_bb_gen_uncaught_set_done
#             else:
#                 jump _dp_bb_gen_uncaught_raise
#     block _dp_bb_gen_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_store_cell(_dp_cell__dp_yield_from_iter_2, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_y_3, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_5, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_raise_7, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_exc_6, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_sent_4, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_close_8, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yield_from_throw_9, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, __dp_DELETED)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_29)
#         jump _dp_bb_gen_uncaught_raise
#     block _dp_bb_gen_uncaught_raise:
#         raise _dp_uncaught_exc_29

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "gen", __dp_make_function("start", 0, "gen", "gen", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     entry_liveins: [_dp_try_exc_2]
#     cellvars: [_dp_try_exc_2->_dp_cell__dp_try_exc_2@deleted]
#     block start:
#         _dp_tmp_1 = Suppress()
#         _dp_with_exit_4 = __dp_contextmanager_get_exit(_dp_tmp_1)
#         __dp_contextmanager_enter(_dp_tmp_1)
#         try_jump:
#             body_label: _dp_bb__dp_module_init_1
#             except_label: _dp_bb__dp_module_init_0
#         block _dp_bb__dp_module_init_1:
#             raise RuntimeError("boom")
#         block _dp_bb__dp_module_init_0:
#             _dp_with_exit_call_5 = _dp_with_exit_4
#             _dp_with_exit_4 = None
#             _dp_tmp_1 = None
#             __dp_contextmanager_exit(_dp_with_exit_call_5, __dp_exc_info())
#             _dp_with_exit_call_5 = None
#             return

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# module_init: _dp_module_init

# function inner()
#     kind: function
#     bind: inner
#     qualname: outer.<locals>.inner
#     entry_liveins: [_dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     block start:
#         return __dp_load_cell(_dp_cell_x)

# function outer()
#     kind: function
#     bind: outer
#     qualname: outer
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         inner = __dp_make_function("start", 0, "inner", "outer.<locals>.inner", __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         return inner()

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "outer", __dp_make_function("start", 1, "outer", "outer", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
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

# function choose(a, b)
#     kind: function
#     bind: choose
#     qualname: choose
#     block start:
#         total = __dp_add(a, b)
#         if_term __dp_gt(total, 5):
#             then:
#                 block _dp_bb_choose_0:
#                     return a
#             else:
#                 block _dp_bb_choose_1:
#                     return b

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "choose", __dp_make_function("start", 0, "choose", "choose", __dp_tuple("a", "b"), __dp_tuple(__dp_tuple("a", None, __dp__.NO_DEFAULT), __dp_tuple("b", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None))
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

# function inner()
#     kind: function
#     bind: inner
#     qualname: outer.<locals>.inner
#     entry_liveins: [_dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     block start:
#         __dp_store_cell(_dp_cell_x, 2)
#         return __dp_load_cell(_dp_cell_x)

# function outer()
#     kind: function
#     bind: outer
#     qualname: outer
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         inner = __dp_make_function("start", 0, "inner", "outer.<locals>.inner", __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         return inner()

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "outer", __dp_make_function("start", 1, "outer", "outer", __dp_tuple(), __dp_tuple(), __dp_globals(), __name__, None, None))
#         return

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# module_init: _dp_module_init

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     entry_liveins: [_dp_try_exc_1]
#     cellvars: [_dp_try_exc_1->_dp_cell__dp_try_exc_1@deleted]
#     block start:
#         try_jump:
#             body_label: _dp_bb__dp_module_init_3
#             except_label: _dp_bb__dp_module_init_2
#         block _dp_bb__dp_module_init_3:
#             print(1)
#             return
#         block _dp_bb__dp_module_init_2:
#             if_term __dp_exception_matches(__dp_current_exception(), Exception):
#                 then:
#                     block _dp_bb__dp_module_init_0:
#                         print(2)
#                         return
#                 else:
#                     block _dp_bb__dp_module_init_1:
#                         raise

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

# function complicated(a)
#     kind: function
#     bind: complicated
#     qualname: complicated
#     local_cell_slots: [_dp_cell__dp_iter_1, _dp_cell__dp_pc, _dp_cell__dp_tmp_2, _dp_cell__dp_try_exc_7, _dp_cell__dp_yieldfrom, _dp_cell_a, _dp_cell_i, _dp_cell_j]
#     cellvars: [a->_dp_cell_a@param, _dp_iter_1->_dp_cell__dp_iter_1@deferred, _dp_try_exc_7->_dp_cell__dp_try_exc_7@deleted, i->_dp_cell_i@deferred, j->_dp_cell_j@deferred, _dp_tmp_2->_dp_cell__dp_tmp_2@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         _dp_cell_a = __dp_make_cell(a)
#         _dp_cell__dp_iter_1 = __dp_make_cell(None)
#         _dp_cell__dp_try_exc_7 = __dp_make_cell(__dp_DELETED)
#         _dp_cell_i = __dp_make_cell(None)
#         _dp_cell_j = __dp_make_cell(None)
#         _dp_cell__dp_tmp_2 = __dp_make_cell(None)
#         _dp_cell__dp_pc = __dp_make_cell(1)
#         _dp_cell__dp_yieldfrom = __dp_make_cell(None)
#         return __dp_make_closure_generator(__dp_def_hidden_resume_fn("start", 1, "_dp_resume", "complicated", __dp_tuple("_dp_self", "_dp_send_value", "_dp_resume_exc", "_dp_cell_a", "_dp_cell__dp_iter_1", "_dp_cell__dp_try_exc_7", "_dp_cell_i", "_dp_cell_j", "_dp_cell__dp_tmp_2", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple("_dp_cell_a", "_dp_cell__dp_iter_1", "_dp_cell__dp_try_exc_7", "_dp_cell_i", "_dp_cell_j", "_dp_cell__dp_tmp_2", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple(_dp_cell_a, _dp_cell__dp_iter_1, _dp_cell__dp_try_exc_7, _dp_cell_i, _dp_cell_j, _dp_cell__dp_tmp_2, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom), __dp_globals(), __name__, async_gen=False), "complicated", "complicated")

# function complicated_resume(a)
#     kind: generator
#     bind: complicated_resume
#     qualname: complicated
#     display_name: _dp_resume
#     entry_liveins: [_dp_self, _dp_send_value, _dp_resume_exc, _dp_cell_a, _dp_cell__dp_iter_1, _dp_cell__dp_try_exc_7, _dp_cell_i, _dp_cell_j, _dp_cell__dp_tmp_2, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     local_cell_slots: [_dp_cell__dp_iter_1, _dp_cell__dp_pc, _dp_cell__dp_tmp_2, _dp_cell__dp_try_exc_7, _dp_cell__dp_yieldfrom, _dp_cell_a, _dp_cell_i, _dp_cell_j]
#     cellvars: [a->_dp_cell_a@param, _dp_iter_1->_dp_cell__dp_iter_1@deferred, _dp_try_exc_7->_dp_cell__dp_try_exc_7@deleted, i->_dp_cell_i@deferred, j->_dp_cell_j@deferred, _dp_tmp_2->_dp_cell__dp_tmp_2@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block _dp_bb_complicated_dispatch_send_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_complicated_done, _dp_bb_complicated_start, _dp_bb_complicated_5] default _dp_bb_complicated_invalid
#                     block _dp_bb_complicated_done:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_complicated_done_return_done
#                         block _dp_bb_complicated_done_return_done:
#                             raise StopIteration()
#                     block _dp_bb_complicated_start:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                         a = __dp_load_deleted_name("a", __dp_load_cell(_dp_cell_a))
#                         __dp_store_cell(_dp_cell_a, a)
#                         _dp_iter_1 = __dp_iter(a)
#                         __dp_store_cell(_dp_cell__dp_iter_1, _dp_iter_1)
#                         jump _dp_bb_complicated_9
#             else:
#                 block _dp_bb_complicated_dispatch_throw_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb_complicated_dispatch_throw_done, _dp_bb_complicated_dispatch_throw_unstarted, _dp_bb_complicated_5] default _dp_bb_complicated_invalid
#                     block _dp_bb_complicated_dispatch_throw_done:
#                         raise _dp_resume_exc
#                     block _dp_bb_complicated_dispatch_throw_unstarted:
#                         raise _dp_resume_exc
#         block _dp_bb_complicated_9:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#             _dp_iter_1 = __dp_load_deleted_name("_dp_iter_1", __dp_load_cell(_dp_cell__dp_iter_1))
#             __dp_store_cell(_dp_cell__dp_iter_1, _dp_iter_1)
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             __dp_store_cell(_dp_cell__dp_tmp_2, _dp_tmp_2)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_complicated_0:
#                         print("finsihed")
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb_complicated_0_return_done
#                         block _dp_bb_complicated_0_return_done:
#                             raise StopIteration()
#                 else:
#                     block _dp_bb_complicated_8:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                         _dp_tmp_2 = __dp_load_deleted_name("_dp_tmp_2", __dp_load_cell(_dp_cell__dp_tmp_2))
#                         __dp_store_cell(_dp_cell__dp_tmp_2, _dp_tmp_2)
#                         i = _dp_tmp_2
#                         __dp_store_cell(_dp_cell_i, i)
#                         _dp_tmp_2 = None
#                         __dp_store_cell(_dp_cell__dp_tmp_2, _dp_tmp_2)
#                         jump _dp_bb_complicated_7
#                         block _dp_bb_complicated_7:
#                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                             try_jump:
#                                 body_label: _dp_bb_complicated_6
#                                 except_label: _dp_bb_complicated_3
#                             block _dp_bb_complicated_6:
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                 i = __dp_load_deleted_name("i", __dp_load_cell(_dp_cell_i))
#                                 __dp_store_cell(_dp_cell_i, i)
#                                 j = __dp_add(i, 1)
#                                 __dp_store_cell(_dp_cell_j, j)
#                                 __dp_store_cell(_dp_cell__dp_pc, 2)
#                                 return j
#                             block _dp_bb_complicated_3:
#                                 __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                 _dp_try_exc_7 = __dp_load_deleted_name("_dp_try_exc_7", __dp_load_cell(_dp_cell__dp_try_exc_7))
#                                 __dp_store_cell(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                 if_term __dp_exception_matches(__dp_current_exception(), Exception):
#                                     then:
#                                         block _dp_bb_complicated_1:
#                                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                             _dp_try_exc_7 = __dp_load_deleted_name("_dp_try_exc_7", __dp_load_cell(_dp_cell__dp_try_exc_7))
#                                             __dp_store_cell(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                             print("oops")
#                                             jump _dp_bb_complicated_9
#                                     else:
#                                         block _dp_bb_complicated_2:
#                                             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                             _dp_try_exc_7 = __dp_load_deleted_name("_dp_try_exc_7", __dp_load_cell(_dp_cell__dp_try_exc_7))
#                                             __dp_store_cell(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                                             raise
#         block _dp_bb_complicated_5:
#             __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb_complicated_4:
#                         __dp_store_cell_if_not_deleted(_dp_cell__dp_try_exc_7, _dp_try_exc_7)
#                         raise _dp_resume_exc
#                 else:
#                     jump _dp_bb_complicated_9
#         block _dp_bb_complicated_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     block _dp_bb_complicated_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 jump _dp_bb_complicated_uncaught_set_done
#             else:
#                 jump _dp_bb_complicated_uncaught_raise
#     block _dp_bb_complicated_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_store_cell(_dp_cell_a, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_iter_1, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_try_exc_7, __dp_DELETED)
#         __dp_store_cell(_dp_cell_i, __dp_DELETED)
#         __dp_store_cell(_dp_cell_j, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_tmp_2, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, __dp_DELETED)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_15)
#         jump _dp_bb_complicated_uncaught_raise
#     block _dp_bb_complicated_uncaught_raise:
#         raise _dp_uncaught_exc_15

# function _dp_module_init()
#     kind: function
#     bind: _dp_module_init
#     qualname: _dp_module_init
#     block start:
#         __dp_store_global(globals(), "complicated", __dp_make_function("start", 0, "complicated", "complicated", __dp_tuple("a"), __dp_tuple(__dp_tuple("a", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None))
#         return
