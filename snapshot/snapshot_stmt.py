# import_simple

import a

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         a = __dp_import_("a", __spec__)
#         return __dp_NONE

# import_dotted_alias

import a.b as c

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         c = __dp_import_attr(__dp_import_("a.b", __spec__), "b")
#         return __dp_NONE

# import_from_alias

from pkg.mod import name as alias

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_import_1 = __dp_import_("pkg.mod", __spec__, ["name"])
#         alias = __dp_import_attr(_dp_import_1, "name")
#         return __dp_NONE

# decorator_function


@dec
def f():
    pass


# ==

# function f():
#     function_id: 0
#     block _dp_bb_0_1:
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         f = dec(__dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# assign_attr

obj.x = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return __dp_NONE

# assign_subscript

obj[i] = v

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)
#         return __dp_NONE

# assign_tuple_unpack

a, b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#         a = __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0)
#         b = __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1)
#         _dp_tmp_1 = __dp_DELETED
#         return __dp_NONE

# assign_star_unpack

a, *b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#         a = __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0)
#         b = __dp_list(__dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1))
#         _dp_tmp_1 = __dp_DELETED
#         return __dp_NONE

# assign_multi_targets

a = b = f()

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_tmp_1 = f()
#         a = _dp_tmp_1
#         b = _dp_tmp_1
#         return __dp_NONE

# ann_assign_simple

x: int = 1

# ==

# function __annotate__(_dp_format, _dp):
#     function_id: 0
#     block _dp_bb_0_1:
#         if_term _dp.eq(_dp_format, 4):
#             then:
#                 block _dp_bb_0_5:
#                     return _dp.dict(__dp_tuple(("x", "int")))
#             else:
#                 block _dp_bb_0_2:
#                     if_term _dp.gt(_dp_format, 2):
#                         then:
#                             block _dp_bb_0_4:
#                                 raise _dp.builtins.NotImplementedError
#                         else:
#                             block _dp_bb_0_3:
#                                 return _dp.dict(__dp_tuple(("x", int)))

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         x = 1
#         __annotate__ = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(__dp__), __dp_globals(), None)
#         return __dp_NONE

# ann_assign_attr

obj.x: int = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return __dp_NONE

# aug_assign_attr

obj.x += 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))
#         return __dp_NONE

# delete_mixed

del obj.x, obj[i], x

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         __dp_delattr(obj, "x")
#         __dp_delitem(obj, i)
#         x = __dp_DELETED
#         return __dp_NONE

# assert_no_msg

assert cond

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         if_term __debug__:
#             then:
#                 block _dp_bb_0_2:
#                     if_term not cond:
#                         then:
#                             block _dp_bb_0_3:
#                                 raise __dp_AssertionError
#                         else:
#                             jump _dp_bb_0_0
#             else:
#                 jump _dp_bb_0_0
#         block _dp_bb_0_0:
#             return __dp_NONE

# assert_with_msg

assert cond, "oops"

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         if_term __debug__:
#             then:
#                 block _dp_bb_0_2:
#                     if_term not cond:
#                         then:
#                             block _dp_bb_0_3:
#                                 raise __dp_AssertionError("oops")
#                         else:
#                             jump _dp_bb_0_0
#             else:
#                 jump _dp_bb_0_0
#         block _dp_bb_0_0:
#             return __dp_NONE

# raise_from

raise E from cause

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         raise __dp_raise_from(E, cause)

# try_except_typed

try:
    f()
except E as e:
    g(e)
except:
    h()

# ==

# snapshot regeneration failed
# panic: py_stmt template must produce exactly one statement, got 2

# for_else

for x in it:
    body()
else:
    done()

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_3:
#         _dp_iter_0_0 = __dp_iter(it)
#         jump _dp_bb_0_1
#         block _dp_bb_0_1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0_4:
#                         done()
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_0_2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump _dp_bb_0_5
#                         block _dp_bb_0_5:
#                             body()
#                             jump _dp_bb_0_1

# while_else

while cond:
    body()
else:
    done()

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         if_term cond:
#             then:
#                 block _dp_bb_0_3:
#                     body()
#                     jump _dp_bb_0_1
#             else:
#                 block _dp_bb_0_2:
#                     done()
#                     return __dp_NONE

# with_as

with cm as x:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_0_3, _dp_try_exc_0_0, _dp_try_abrupt_kind_0_1]
#     block _dp_bb_0_4:
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
#         x = __dp_contextmanager_enter(cm)
#         _dp_with_ok_2 = True
#         jump _dp_bb_0_11
#         block _dp_bb_0_11:
#             body()
#             jump _dp_bb_0_5__normal
#             block _dp_bb_0_5__normal:
#                 jump _dp_bb_0_5(Fallthrough, None)
#                 block _dp_bb_0_5:
#                     exc_param: _dp_try_exc_0_3
#                     params: [_dp_try_abrupt_kind_0_1:AbruptKind, _dp_try_abrupt_payload_0_2:AbruptPayload, _dp_try_exc_0_3:Exception]
#                     if_term _dp_with_ok_2:
#                         then:
#                             block _dp_bb_0_7:
#                                 exc_param: _dp_try_exc_0_3
#                                 params: [_dp_try_exc_0_3:Exception]
#                                 __dp_contextmanager_exit(_dp_with_exit_1, None)
#                                 jump _dp_bb_0_6
#                         else:
#                             jump _dp_bb_0_6
#                     block _dp_bb_0_6:
#                         exc_param: _dp_try_exc_0_3
#                         params: [_dp_try_exc_0_3:Exception]
#                         _dp_with_exit_1 = None
#                         jump _dp_bb_0_1
#                         block _dp_bb_0_1:
#                             branch_table _dp_try_abrupt_kind_0_1 -> [_dp_bb_0_0, _dp_bb_0_2, _dp_bb_0_3] default _dp_bb_0_0
#                             block _dp_bb_0_0:
#                                 return __dp_NONE
#                             block _dp_bb_0_2:
#                                 return _dp_try_abrupt_payload_0_2
#                             block _dp_bb_0_3:
#                                 raise _dp_try_abrupt_payload_0_2
#     block _dp_bb_0_10:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         raise
#     block _dp_bb_0_5__exception:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_exc_0_3:Exception]
#         jump _dp_bb_0_5(Exception, _dp_try_exc_0_3)
#     block _dp_bb_0_8:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#             then:
#                 jump _dp_bb_0_9
#             else:
#                 jump _dp_bb_0_10
#     block _dp_bb_0_9:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#         jump _dp_bb_0_5__normal

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# function inner():
#     function_id: 0
#     block _dp_bb_0_1:
#         value = 1
#         return value

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         inner = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# function _dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb_0_3:
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_0_1
#         block _dp_bb_0_1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0_4:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_0_2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump _dp_bb_0_5
#                         block _dp_bb_0_5:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb_0_1

# function _dp_setcomp_6(_dp_iter_5):
#     function_id: 1
#     display_name: <setcomp>
#     block _dp_bb_1_3:
#         _dp_tmp_4 = set()
#         _dp_iter_1_0 = __dp_iter(_dp_iter_5)
#         jump _dp_bb_1_1
#         block _dp_bb_1_1:
#             _dp_tmp_1_1 = __dp_next_or_sentinel(_dp_iter_1_0)
#             if_term __dp_is_(_dp_tmp_1_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_1_4:
#                         return _dp_tmp_4
#                 else:
#                     block _dp_bb_1_2:
#                         x = _dp_tmp_1_1
#                         _dp_tmp_1_1 = None
#                         jump _dp_bb_1_5
#                         block _dp_bb_1_5:
#                             _dp_tmp_4.add(x)
#                             jump _dp_bb_1_1

# function _dp_dictcomp_9(_dp_iter_8):
#     function_id: 2
#     display_name: <dictcomp>
#     block _dp_bb_2_3:
#         _dp_tmp_7 = {}
#         _dp_iter_2_0 = __dp_iter(_dp_iter_8)
#         jump _dp_bb_2_1
#         block _dp_bb_2_1:
#             _dp_tmp_2_1 = __dp_next_or_sentinel(_dp_iter_2_0)
#             if_term __dp_is_(_dp_tmp_2_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_2_4:
#                         return _dp_tmp_7
#                 else:
#                     block _dp_bb_2_2:
#                         _dp_tmp_2_2 = __dp_unpack(_dp_tmp_2_1, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_2_2, 0)
#                         v = __dp_getitem(_dp_tmp_2_2, 1)
#                         del _dp_tmp_2_2
#                         _dp_tmp_2_1 = None
#                         jump _dp_bb_2_5
#                         block _dp_bb_2_5:
#                             __dp_setitem(_dp_tmp_7, k, v)
#                             jump _dp_bb_2_1

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_listcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         xs = _dp_listcomp_3(it)
#         _dp_setcomp_6 = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         ys = _dp_setcomp_6(it)
#         _dp_dictcomp_9 = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         zs = _dp_dictcomp_9(items)
#         return __dp_NONE

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# function f.<locals>._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb_0_3:
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_0_1
#         block _dp_bb_0_1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0_4:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_0_2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump _dp_bb_0_5
#                         block _dp_bb_0_5:
#                             if_term __dp_gt(x, 0):
#                                 then:
#                                     block _dp_bb_0_6:
#                                         _dp_tmp_1.append(x)
#                                         jump _dp_bb_0_1
#                                 else:
#                                     jump _dp_bb_0_1

# function f():
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_listcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return _dp_listcomp_3(it)

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_2_1:
#         f = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# function C._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb_0_3:
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_0_1
#         block _dp_bb_0_1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0_4:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_0_2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump _dp_bb_0_5
#                         block _dp_bb_0_5:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb_0_1

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block _dp_bb_1_1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         _dp_listcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "xs", _dp_listcomp_3(it))
#         return __dp_NONE

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block _dp_bb_2_1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_3_1:
#         _dp_class_ns_C = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_C = __dp_make_function(2, "function", __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         C = _dp_define_class_C(_dp_class_ns_C, globals())
#         return __dp_NONE

# with_multi

with a as x, b as y:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_0_3, _dp_try_exc_0_0, _dp_try_exc_0_7, _dp_try_exc_0_4, _dp_try_abrupt_kind_0_1, _dp_try_abrupt_kind_0_5]
#     block _dp_bb_0_4:
#         _dp_with_exit_4 = __dp_contextmanager_get_exit(a)
#         x = __dp_contextmanager_enter(a)
#         _dp_with_ok_5 = True
#         jump _dp_bb_0_14
#         block _dp_bb_0_14:
#             _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
#             y = __dp_contextmanager_enter(b)
#             _dp_with_ok_2 = True
#             jump _dp_bb_0_21
#             block _dp_bb_0_21:
#                 body()
#                 jump _dp_bb_0_15__normal
#                 block _dp_bb_0_15__normal:
#                     jump _dp_bb_0_15(Fallthrough, None)
#                     block _dp_bb_0_15:
#                         exc_param: _dp_try_exc_0_7
#                         params: [_dp_try_abrupt_kind_0_5:AbruptKind, _dp_try_abrupt_payload_0_6:AbruptPayload, _dp_try_exc_0_7:Exception]
#                         if_term _dp_with_ok_2:
#                             then:
#                                 block _dp_bb_0_17:
#                                     exc_param: _dp_try_exc_0_7
#                                     params: [_dp_try_exc_0_7:Exception]
#                                     __dp_contextmanager_exit(_dp_with_exit_1, None)
#                                     jump _dp_bb_0_16
#                             else:
#                                 jump _dp_bb_0_16
#                         block _dp_bb_0_16:
#                             exc_param: _dp_try_exc_0_7
#                             params: [_dp_try_exc_0_7:Exception]
#                             _dp_with_exit_1 = None
#                             jump _dp_bb_0_11
#                             block _dp_bb_0_11:
#                                 branch_table _dp_try_abrupt_kind_0_5 -> [_dp_bb_0_5__normal, _dp_bb_0_12, _dp_bb_0_13] default _dp_bb_0_5__normal
#                                 block _dp_bb_0_12:
#                                     _dp_try_abrupt_payload_0_2 = _dp_try_abrupt_payload_0_6
#                                     jump _dp_bb_0_5(Return, _dp_try_abrupt_payload_0_2)
#                                 block _dp_bb_0_13:
#                                     raise _dp_try_abrupt_payload_0_6
#                                 block _dp_bb_0_5:
#                                     exc_param: _dp_try_exc_0_3
#                                     params: [_dp_try_abrupt_kind_0_1:AbruptKind, _dp_try_abrupt_payload_0_2:AbruptPayload, _dp_try_exc_0_3:Exception]
#                                     if_term _dp_with_ok_5:
#                                         then:
#                                             block _dp_bb_0_7:
#                                                 exc_param: _dp_try_exc_0_3
#                                                 params: [_dp_try_exc_0_3:Exception]
#                                                 __dp_contextmanager_exit(_dp_with_exit_4, None)
#                                                 jump _dp_bb_0_6
#                                         else:
#                                             jump _dp_bb_0_6
#                                     block _dp_bb_0_6:
#                                         exc_param: _dp_try_exc_0_3
#                                         params: [_dp_try_exc_0_3:Exception]
#                                         _dp_with_exit_4 = None
#                                         jump _dp_bb_0_1
#                                         block _dp_bb_0_1:
#                                             branch_table _dp_try_abrupt_kind_0_1 -> [_dp_bb_0_0, _dp_bb_0_2, _dp_bb_0_3] default _dp_bb_0_0
#                                             block _dp_bb_0_0:
#                                                 return __dp_NONE
#                                             block _dp_bb_0_2:
#                                                 return _dp_try_abrupt_payload_0_2
#                                             block _dp_bb_0_3:
#                                                 raise _dp_try_abrupt_payload_0_2
#                                 block _dp_bb_0_5__normal:
#                                     jump _dp_bb_0_5(Fallthrough, None)
#     block _dp_bb_0_10:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         raise
#     block _dp_bb_0_15__exception:
#         exc_param: _dp_try_exc_0_7
#         params: [_dp_try_exc_0_7:Exception]
#         jump _dp_bb_0_15(Exception, _dp_try_exc_0_7)
#     block _dp_bb_0_18:
#         exc_param: _dp_try_exc_0_4
#         params: [_dp_try_exc_0_4:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#             then:
#                 jump _dp_bb_0_19
#             else:
#                 jump _dp_bb_0_20
#     block _dp_bb_0_19:
#         exc_param: _dp_try_exc_0_4
#         params: [_dp_try_exc_0_4:Exception]
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#         jump _dp_bb_0_15__normal
#     block _dp_bb_0_20:
#         exc_param: _dp_try_exc_0_4
#         params: [_dp_try_exc_0_4:Exception]
#         raise
#     block _dp_bb_0_5__exception:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_exc_0_3:Exception]
#         jump _dp_bb_0_5(Exception, _dp_try_exc_0_3)
#     block _dp_bb_0_8:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#             then:
#                 jump _dp_bb_0_9
#             else:
#                 jump _dp_bb_0_10
#     block _dp_bb_0_9:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         _dp_with_ok_5 = False
#         __dp_contextmanager_exit(_dp_with_exit_4, __dp_exc_info())
#         jump _dp_bb_0_5__normal

# async_for


async def run():
    async for x in ait:
        body()


# ==

# coroutine run():
#     function_id: 0
#     block _dp_bb_0_3:
#         _dp_iter_0_0 = __dp_aiter(ait)
#         jump _dp_bb_0_1
#         block _dp_bb_0_1:
#             _dp_tmp_0_1 = await __dp_anext_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0_0:
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_0_2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump _dp_bb_0_4
#                         block _dp_bb_0_4:
#                             body()
#                             jump _dp_bb_0_1

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         run = __dp_make_function(0, "coroutine", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# async_with


async def run():
    async with cm as x:
        body()


# ==

# coroutine run():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_0_3, _dp_try_exc_0_0, _dp_try_abrupt_kind_0_1]
#     block _dp_bb_0_4:
#         _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
#         x = await __dp_asynccontextmanager_aenter(cm)
#         _dp_with_ok_2 = True
#         jump _dp_bb_0_12
#         block _dp_bb_0_12:
#             body()
#             jump _dp_bb_0_5__normal
#             block _dp_bb_0_5__normal:
#                 jump _dp_bb_0_5(Fallthrough, None)
#                 block _dp_bb_0_5:
#                     exc_param: _dp_try_exc_0_3
#                     params: [_dp_try_abrupt_kind_0_1:AbruptKind, _dp_try_abrupt_payload_0_2:AbruptPayload, _dp_try_exc_0_3:Exception]
#                     if_term _dp_with_ok_2:
#                         then:
#                             block _dp_bb_0_7:
#                                 exc_param: _dp_try_exc_0_3
#                                 params: [_dp_try_exc_0_3:Exception]
#                                 await __dp_asynccontextmanager_exit(_dp_with_exit_1, None)
#                                 jump _dp_bb_0_6
#                         else:
#                             jump _dp_bb_0_6
#                     block _dp_bb_0_6:
#                         exc_param: _dp_try_exc_0_3
#                         params: [_dp_try_exc_0_3:Exception]
#                         _dp_with_exit_1 = None
#                         jump _dp_bb_0_1
#                         block _dp_bb_0_1:
#                             branch_table _dp_try_abrupt_kind_0_1 -> [_dp_bb_0_0, _dp_bb_0_2, _dp_bb_0_3] default _dp_bb_0_0
#                             block _dp_bb_0_0:
#                                 return __dp_NONE
#                             block _dp_bb_0_2:
#                                 return _dp_try_abrupt_payload_0_2
#                             block _dp_bb_0_3:
#                                 raise _dp_try_abrupt_payload_0_2
#     block _dp_bb_0_10:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         raise _dp_with_reraise_3
#     block _dp_bb_0_11:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         raise
#     block _dp_bb_0_5__exception:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_exc_0_3:Exception]
#         jump _dp_bb_0_5(Exception, _dp_try_exc_0_3)
#     block _dp_bb_0_8:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#             then:
#                 jump _dp_bb_0_9
#             else:
#                 jump _dp_bb_0_11
#     block _dp_bb_0_9:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         _dp_with_ok_2 = False
#         _dp_with_reraise_3 = await __dp_asynccontextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#         if_term __dp_is_not(_dp_with_reraise_3, None):
#             then:
#                 jump _dp_bb_0_10
#             else:
#                 jump _dp_bb_0_5__normal

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         run = __dp_make_function(0, "coroutine", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_0_1:
#         _dp_match_1 = value
#         if_term __dp_eq(_dp_match_1, 1):
#             then:
#                 block _dp_bb_0_2:
#                     one()
#                     return __dp_NONE
#             else:
#                 block _dp_bb_0_3:
#                     other()
#                     return __dp_NONE

# generator_yield


def gen():
    yield 1


# ==

# generator gen():
#     function_id: 0
#     block _dp_bb_0_1:
#         yield 1
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         gen = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# yield_from


def gen():
    yield from it


# ==

# generator gen():
#     function_id: 0
#     block _dp_bb_0_1:
#         yield from it
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         gen = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_0_3, _dp_try_exc_0_0, _dp_try_abrupt_kind_0_1]
#     block _dp_bb_0_4:
#         _dp_tmp_4 = Suppress()
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_4)
#         __dp_contextmanager_enter(_dp_tmp_4)
#         _dp_with_ok_2 = True
#         jump _dp_bb_0_11
#         block _dp_bb_0_11:
#             raise RuntimeError("boom")
#     block _dp_bb_0_0:
#         return __dp_NONE
#     block _dp_bb_0_1:
#         branch_table _dp_try_abrupt_kind_0_1 -> [_dp_bb_0_0, _dp_bb_0_2, _dp_bb_0_3] default _dp_bb_0_0
#     block _dp_bb_0_10:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         raise
#     block _dp_bb_0_2:
#         return _dp_try_abrupt_payload_0_2
#     block _dp_bb_0_3:
#         raise _dp_try_abrupt_payload_0_2
#     block _dp_bb_0_5:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_abrupt_kind_0_1:AbruptKind, _dp_try_abrupt_payload_0_2:AbruptPayload, _dp_try_exc_0_3:Exception]
#         if_term _dp_with_ok_2:
#             then:
#                 jump _dp_bb_0_7
#             else:
#                 jump _dp_bb_0_6
#     block _dp_bb_0_5__exception:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_exc_0_3:Exception]
#         jump _dp_bb_0_5(Exception, _dp_try_exc_0_3)
#     block _dp_bb_0_5__normal:
#         jump _dp_bb_0_5(Fallthrough, None)
#     block _dp_bb_0_6:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_exc_0_3:Exception]
#         _dp_with_exit_1 = None
#         _dp_tmp_4 = None
#         jump _dp_bb_0_1
#     block _dp_bb_0_7:
#         exc_param: _dp_try_exc_0_3
#         params: [_dp_try_exc_0_3:Exception]
#         __dp_contextmanager_exit(_dp_with_exit_1, None)
#         jump _dp_bb_0_6
#     block _dp_bb_0_8:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#             then:
#                 jump _dp_bb_0_9
#             else:
#                 jump _dp_bb_0_10
#     block _dp_bb_0_9:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#         jump _dp_bb_0_5__normal

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# function outer.<locals>.inner():
#     function_id: 0
#     freevars: [x->_dp_cell_x@inherited]
#     block _dp_bb_0_1:
#         return x

# function outer():
#     function_id: 1
#     local_cell_slots: [_dp_cell_x, _dp_cell_inner]
#     cellvars: [x->_dp_cell_x@deferred, inner->_dp_cell_inner@deferred]
#     block _dp_bb_1_1:
#         _dp_cell_x = __dp_make_cell()
#         x = 5
#         inner = __dp_make_function(0, "function", __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), None)
#         return inner()

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_2_1:
#         outer = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==

# function choose(a, b):
#     function_id: 0
#     block _dp_bb_0_1:
#         total = a + b
#         if_term __dp_gt(total, 5):
#             then:
#                 block _dp_bb_0_2:
#                     return a
#             else:
#                 block _dp_bb_0_3:
#                     return b

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         choose = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==

# function outer.<locals>.inner():
#     function_id: 0
#     freevars: [x->_dp_cell_x@inherited]
#     block _dp_bb_0_1:
#         x = 2
#         return x

# function outer():
#     function_id: 1
#     local_cell_slots: [_dp_cell_x, _dp_cell_inner]
#     cellvars: [x->_dp_cell_x@deferred, inner->_dp_cell_inner@deferred]
#     block _dp_bb_1_1:
#         _dp_cell_x = __dp_make_cell()
#         x = 5
#         inner = __dp_make_function(0, "function", __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), None)
#         return inner()

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_2_1:
#         outer = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_0_0]
#     block _dp_bb_0_1:
#         jump _dp_bb_0_5
#         block _dp_bb_0_5:
#             print(1)
#             return __dp_NONE
#     block _dp_bb_0_2:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), Exception):
#             then:
#                 jump _dp_bb_0_3
#             else:
#                 jump _dp_bb_0_4
#     block _dp_bb_0_3:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         print(2)
#         return __dp_NONE
#     block _dp_bb_0_4:
#         exc_param: _dp_try_exc_0_0
#         params: [_dp_try_exc_0_0:Exception]
#         raise

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

# generator complicated(a):
#     function_id: 0
#     entry_liveins: [a, _dp_try_exc_0_2]
#     block _dp_bb_0_3:
#         _dp_iter_0_0 = __dp_iter(a)
#         jump _dp_bb_0_1
#         block _dp_bb_0_1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0_4:
#                         print("finsihed")
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_0_2:
#                         i = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump _dp_bb_0_5
#                         block _dp_bb_0_5:
#                             jump _dp_bb_0_9
#                             block _dp_bb_0_9:
#                                 j = i + 1
#                                 yield j
#                                 jump _dp_bb_0_1
#     block _dp_bb_0_6:
#         exc_param: _dp_try_exc_0_2
#         params: [_dp_try_exc_0_2:Exception]
#         if_term __dp_exception_matches(__dp_current_exception(), Exception):
#             then:
#                 jump _dp_bb_0_7
#             else:
#                 jump _dp_bb_0_8
#     block _dp_bb_0_7:
#         exc_param: _dp_try_exc_0_2
#         params: [_dp_try_exc_0_2:Exception]
#         print("oops")
#         jump _dp_bb_0_1
#     block _dp_bb_0_8:
#         exc_param: _dp_try_exc_0_2
#         params: [_dp_try_exc_0_2:Exception]
#         raise

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_1_1:
#         complicated = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE
