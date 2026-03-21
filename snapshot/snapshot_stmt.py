# import_simple

import a

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_store_global(globals(), "a", __dp_import_("a", __spec__))
#         return __dp_NONE

# import_dotted_alias

import a.b as c

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_store_global(globals(), "c", __dp_import_attr(__dp_import_("a.b", __spec__), "b"))
#         return __dp_NONE

# import_from_alias

from pkg.mod import name as alias

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         _dp_import_1 = __dp_import_("pkg.mod", __spec__, ["name"])
#         __dp_store_global(globals(), "alias", __dp_import_attr(_dp_import_1, "name"))
#         return __dp_NONE

# decorator_function


@dec
def f():
    pass


# ==

# function f():
#     function_id: 0
#     block _dp_bb_start:
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "f", dec(__dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)))
#         return __dp_NONE

# assign_attr

obj.x = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return __dp_NONE

# assign_subscript

obj[i] = v

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)
#         return __dp_NONE

# assign_tuple_unpack

a, b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#         __dp_store_global(globals(), "a", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0))
#         __dp_store_global(globals(), "b", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1))
#         _dp_tmp_1 = __dp_DELETED
#         return __dp_NONE

# assign_star_unpack

a, *b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#         __dp_store_global(globals(), "a", __dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0))
#         __dp_store_global(globals(), "b", __dp_list(__dp_getitem(__dp_load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1)))
#         _dp_tmp_1 = __dp_DELETED
#         return __dp_NONE

# assign_multi_targets

a = b = f()

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         _dp_tmp_1 = f()
#         __dp_store_global(globals(), "a", _dp_tmp_1)
#         __dp_store_global(globals(), "b", _dp_tmp_1)
#         return __dp_NONE

# ann_assign_simple

x: int = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_store_global(globals(), "x", 1)
#         __annotate__ = __dp_exec_function_def_source('def __annotate__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_tuple=__dp_tuple):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(__dp_tuple(("x", "int")))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(__dp_tuple(("x", int)))', __dp_globals(), __dp_tuple(), "__annotate__")
#         __dp_store_global(globals(), "__annotate__", __dp_update_fn(__annotate__, "__annotate__", "__annotate__", None))
#         return __dp_NONE

# ann_assign_attr

obj.x: int = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return __dp_NONE

# aug_assign_attr

obj.x += 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))
#         return __dp_NONE

# delete_mixed

del obj.x, obj[i], x

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         __dp_delattr(obj, "x")
#         __dp_delitem(obj, i)
#         __dp_delitem(globals(), "x")
#         return __dp_NONE

# assert_no_msg

assert cond

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         if_term __debug__:
#             then:
#                 block _dp_bb_1:
#                     if_term not cond:
#                         then:
#                             block _dp_bb_0:
#                                 raise __dp_AssertionError
#                         else:
#                             jump _dp_bb_2
#             else:
#                 jump _dp_bb_2
#         block _dp_bb_2:
#             return __dp_NONE

# assert_with_msg

assert cond, "oops"

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         if_term __debug__:
#             then:
#                 block _dp_bb_1:
#                     if_term not cond:
#                         then:
#                             block _dp_bb_0:
#                                 raise __dp_AssertionError("oops")
#                         else:
#                             jump _dp_bb_2
#             else:
#                 jump _dp_bb_2
#         block _dp_bb_2:
#             return __dp_NONE

# raise_from

raise E from cause

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
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
#     block _dp_bb_start:
#         _dp_iter_1 = __dp_iter(it)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         done()
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             __dp_store_global(globals(), "x", x)
#                             body()
#                             jump _dp_bb_3

# while_else

while cond:
    body()
else:
    done()

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb_start:
#         if_term cond:
#             then:
#                 block _dp_bb_1:
#                     body()
#                     jump _dp_bb_start
#             else:
#                 block _dp_bb_0:
#                     done()
#                     return __dp_NONE

# with_as

with cm as x:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_7, _dp_try_exc_1, _dp_try_abrupt_kind_2]
#     block _dp_bb_start:
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
#         x = __dp_contextmanager_enter(cm)
#         _dp_with_ok_2 = True
#         try_jump:
#             body_label: _dp_bb_11
#             except_label: _dp_bb_10
#         block _dp_bb_10:
#             exc_param: _dp_try_exc_1
#             params: [_dp_try_exc_1:Exception]
#             if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#                 then:
#                     block _dp_bb_8:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         _dp_with_ok_2 = False
#                         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#                         jump _dp_bb_3
#                 else:
#                     block _dp_bb_9:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         raise
#         block _dp_bb_11:
#             body()
#             jump _dp_bb_3
#         block _dp_bb_3:
#             jump _dp_bb_2(Fallthrough, None)
#             block _dp_bb_2:
#                 exc_param: _dp_try_exc_7
#                 params: [_dp_try_abrupt_kind_2:AbruptKind, _dp_try_abrupt_payload_3:AbruptPayload, _dp_try_exc_7:Exception]
#                 if_term _dp_with_ok_2:
#                     then:
#                         block _dp_bb_1:
#                             exc_param: _dp_try_exc_7
#                             params: [_dp_try_exc_7:Exception]
#                             __dp_contextmanager_exit(_dp_with_exit_1, None)
#                             jump _dp_bb_0
#                     else:
#                         jump _dp_bb_0
#                 block _dp_bb_0:
#                     exc_param: _dp_try_exc_7
#                     params: [_dp_try_exc_7:Exception]
#                     _dp_with_exit_1 = None
#                     jump _dp_bb_7
#                     block _dp_bb_7:
#                         branch_table _dp_try_abrupt_kind_2 -> [_dp_bb_12, _dp_bb_5, _dp_bb_6] default _dp_bb_12
#                         block _dp_bb_12:
#                             return __dp_NONE
#                         block _dp_bb_5:
#                             return _dp_try_abrupt_payload_3
#                         block _dp_bb_6:
#                             raise _dp_try_abrupt_payload_3
#     block _dp_bb_4:
#         exc_param: _dp_try_exc_7
#         params: [_dp_try_exc_7:Exception]
#         jump _dp_bb_2(Exception, _dp_try_exc_7)

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# function inner():
#     function_id: 0
#     block _dp_bb_start:
#         value = 1
#         return value

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "inner", __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# function _dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb_start:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb_3

# function _dp_setcomp_6(_dp_iter_5):
#     function_id: 1
#     display_name: <setcomp>
#     block _dp_bb_start:
#         _dp_tmp_4 = set()
#         _dp_iter_9 = __dp_iter(_dp_iter_5)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_10 = __dp_next_or_sentinel(_dp_iter_9)
#             if_term __dp_is_(_dp_tmp_10, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_4
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_10
#                         _dp_tmp_10 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_4.add(x)
#                             jump _dp_bb_3

# function _dp_dictcomp_9(_dp_iter_8):
#     function_id: 2
#     display_name: <dictcomp>
#     block _dp_bb_start:
#         _dp_tmp_7 = {}
#         _dp_iter_17 = __dp_iter(_dp_iter_8)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_18 = __dp_next_or_sentinel(_dp_iter_17)
#             if_term __dp_is_(_dp_tmp_18, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_7
#                 else:
#                     block _dp_bb_2:
#                         _dp_tmp_20 = __dp_unpack(_dp_tmp_18, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_20, 0)
#                         v = __dp_getitem(_dp_tmp_20, 1)
#                         del _dp_tmp_20
#                         _dp_tmp_18 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             __dp_setitem(_dp_tmp_7, k, v)
#                             jump _dp_bb_3

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_start:
#         _dp_listcomp_3 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "xs", _dp_listcomp_3(it))
#         _dp_setcomp_6 = __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "ys", _dp_setcomp_6(it))
#         _dp_dictcomp_9 = __dp_make_function(2, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "zs", _dp_dictcomp_9(items))
#         return __dp_NONE

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# function f.<locals>._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb_start:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_4
#         block _dp_bb_4:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_3:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_2
#                         block _dp_bb_2:
#                             if_term __dp_gt(x, 0):
#                                 then:
#                                     block _dp_bb_1:
#                                         _dp_tmp_1.append(x)
#                                         jump _dp_bb_4
#                                 else:
#                                     jump _dp_bb_4

# function f():
#     function_id: 1
#     block _dp_bb_start:
#         _dp_listcomp_3 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return _dp_listcomp_3(it)

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_start:
#         __dp_store_global(globals(), "f", __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# function C._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb_start:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_1.append(x)
#                             jump _dp_bb_3

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block _dp_bb_start:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         _dp_listcomp_3 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "xs", _dp_listcomp_3(__dp_class_lookup_global(_dp_class_ns, "it", globals())))
#         return __dp_NONE

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict=None):
#     function_id: 2
#     block _dp_bb_start:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block _dp_bb_start:
#         _dp_class_ns_C = __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         _dp_define_class_C = __dp_make_function(2, __dp_tuple(), __dp_tuple(None), __dp_globals(), None)
#         __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))
#         return __dp_NONE

# with_multi

with a as x, b as y:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_7, _dp_try_exc_1, _dp_try_exc_21, _dp_try_exc_15, _dp_try_abrupt_kind_2, _dp_try_abrupt_kind_16]
#     block _dp_bb_start:
#         _dp_with_exit_4 = __dp_contextmanager_get_exit(a)
#         x = __dp_contextmanager_enter(a)
#         _dp_with_ok_5 = True
#         try_jump:
#             body_label: _dp_bb_23
#             except_label: _dp_bb_10
#         block _dp_bb_10:
#             exc_param: _dp_try_exc_1
#             params: [_dp_try_exc_1:Exception]
#             if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#                 then:
#                     block _dp_bb_8:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         _dp_with_ok_5 = False
#                         __dp_contextmanager_exit(_dp_with_exit_4, __dp_exc_info())
#                         jump _dp_bb_3
#                 else:
#                     block _dp_bb_9:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         raise
#         block _dp_bb_2:
#             exc_param: _dp_try_exc_7
#             params: [_dp_try_abrupt_kind_2:AbruptKind, _dp_try_abrupt_payload_3:AbruptPayload, _dp_try_exc_7:Exception]
#             if_term _dp_with_ok_5:
#                 then:
#                     block _dp_bb_1:
#                         exc_param: _dp_try_exc_7
#                         params: [_dp_try_exc_7:Exception]
#                         __dp_contextmanager_exit(_dp_with_exit_4, None)
#                         jump _dp_bb_0
#                 else:
#                     jump _dp_bb_0
#             block _dp_bb_0:
#                 exc_param: _dp_try_exc_7
#                 params: [_dp_try_exc_7:Exception]
#                 _dp_with_exit_4 = None
#                 jump _dp_bb_7
#                 block _dp_bb_7:
#                     branch_table _dp_try_abrupt_kind_2 -> [_dp_bb_24, _dp_bb_5, _dp_bb_6] default _dp_bb_24
#                     block _dp_bb_24:
#                         return __dp_NONE
#                     block _dp_bb_5:
#                         return _dp_try_abrupt_payload_3
#                     block _dp_bb_6:
#                         raise _dp_try_abrupt_payload_3
#         block _dp_bb_23:
#             _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
#             y = __dp_contextmanager_enter(b)
#             _dp_with_ok_2 = True
#             try_jump:
#                 body_label: _dp_bb_22
#                 except_label: _dp_bb_21
#             block _dp_bb_14:
#                 jump _dp_bb_13(Fallthrough, None)
#                 block _dp_bb_13:
#                     exc_param: _dp_try_exc_21
#                     params: [_dp_try_abrupt_kind_16:AbruptKind, _dp_try_abrupt_payload_17:AbruptPayload, _dp_try_exc_21:Exception]
#                     if_term _dp_with_ok_2:
#                         then:
#                             block _dp_bb_12:
#                                 exc_param: _dp_try_exc_21
#                                 params: [_dp_try_exc_21:Exception]
#                                 __dp_contextmanager_exit(_dp_with_exit_1, None)
#                                 jump _dp_bb_11
#                         else:
#                             jump _dp_bb_11
#                     block _dp_bb_11:
#                         exc_param: _dp_try_exc_21
#                         params: [_dp_try_exc_21:Exception]
#                         _dp_with_exit_1 = None
#                         jump _dp_bb_18
#                         block _dp_bb_18:
#                             branch_table _dp_try_abrupt_kind_16 -> [_dp_bb_3, _dp_bb_16, _dp_bb_17] default _dp_bb_3
#                             block _dp_bb_16:
#                                 _dp_try_abrupt_payload_3 = _dp_try_abrupt_payload_17
#                                 jump _dp_bb_2(Return, _dp_try_abrupt_payload_3)
#                             block _dp_bb_17:
#                                 raise _dp_try_abrupt_payload_17
#             block _dp_bb_21:
#                 exc_param: _dp_try_exc_15
#                 params: [_dp_try_exc_15:Exception]
#                 if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#                     then:
#                         block _dp_bb_19:
#                             exc_param: _dp_try_exc_15
#                             params: [_dp_try_exc_15:Exception]
#                             _dp_with_ok_2 = False
#                             __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#                             jump _dp_bb_14
#                     else:
#                         block _dp_bb_20:
#                             exc_param: _dp_try_exc_15
#                             params: [_dp_try_exc_15:Exception]
#                             raise
#             block _dp_bb_22:
#                 body()
#                 jump _dp_bb_14
#         block _dp_bb_3:
#             jump _dp_bb_2(Fallthrough, None)
#     block _dp_bb_15:
#         exc_param: _dp_try_exc_21
#         params: [_dp_try_exc_21:Exception]
#         jump _dp_bb_13(Exception, _dp_try_exc_21)
#     block _dp_bb_4:
#         exc_param: _dp_try_exc_7
#         params: [_dp_try_exc_7:Exception]
#         jump _dp_bb_2(Exception, _dp_try_exc_7)

# async_for


async def run():
    async for x in ait:
        body()


# ==

# coroutine run():
#     function_id: 0
#     block _dp_bb_start:
#         _dp_iter_1 = __dp_aiter(ait)
#         jump _dp_bb_2
#         block _dp_bb_2:
#             _dp_tmp_2 = await __dp_anext_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_3:
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_1:
#                         x = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_0
#                         block _dp_bb_0:
#                             body()
#                             jump _dp_bb_2

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "run", __dp_mark_coroutine_function(__dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)))
#         return __dp_NONE

# async_with


async def run():
    async with cm as x:
        body()


# ==

# coroutine run():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_7, _dp_try_exc_1, _dp_try_abrupt_kind_2]
#     block _dp_bb_start:
#         _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
#         x = await __dp_asynccontextmanager_aenter(cm)
#         _dp_with_ok_2 = True
#         try_jump:
#             body_label: _dp_bb_12
#             except_label: _dp_bb_11
#         block _dp_bb_11:
#             exc_param: _dp_try_exc_1
#             params: [_dp_try_exc_1:Exception]
#             if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#                 then:
#                     block _dp_bb_9:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         _dp_with_ok_2 = False
#                         _dp_with_reraise_3 = await __dp_asynccontextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#                         if_term __dp_is_not(_dp_with_reraise_3, None):
#                             then:
#                                 block _dp_bb_8:
#                                     exc_param: _dp_try_exc_1
#                                     params: [_dp_try_exc_1:Exception]
#                                     raise _dp_with_reraise_3
#                             else:
#                                 jump _dp_bb_3
#                 else:
#                     block _dp_bb_10:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         raise
#         block _dp_bb_12:
#             body()
#             jump _dp_bb_3
#         block _dp_bb_3:
#             jump _dp_bb_2(Fallthrough, None)
#             block _dp_bb_2:
#                 exc_param: _dp_try_exc_7
#                 params: [_dp_try_abrupt_kind_2:AbruptKind, _dp_try_abrupt_payload_3:AbruptPayload, _dp_try_exc_7:Exception]
#                 if_term _dp_with_ok_2:
#                     then:
#                         block _dp_bb_1:
#                             exc_param: _dp_try_exc_7
#                             params: [_dp_try_exc_7:Exception]
#                             await __dp_asynccontextmanager_exit(_dp_with_exit_1, None)
#                             jump _dp_bb_0
#                     else:
#                         jump _dp_bb_0
#                 block _dp_bb_0:
#                     exc_param: _dp_try_exc_7
#                     params: [_dp_try_exc_7:Exception]
#                     _dp_with_exit_1 = None
#                     jump _dp_bb_7
#                     block _dp_bb_7:
#                         branch_table _dp_try_abrupt_kind_2 -> [_dp_bb_13, _dp_bb_5, _dp_bb_6] default _dp_bb_13
#                         block _dp_bb_13:
#                             return __dp_NONE
#                         block _dp_bb_5:
#                             return _dp_try_abrupt_payload_3
#                         block _dp_bb_6:
#                             raise _dp_try_abrupt_payload_3
#     block _dp_bb_4:
#         exc_param: _dp_try_exc_7
#         params: [_dp_try_exc_7:Exception]
#         jump _dp_bb_2(Exception, _dp_try_exc_7)

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "run", __dp_mark_coroutine_function(__dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)))
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
#     block _dp_bb_start:
#         _dp_match_1 = value
#         if_term __dp_eq(_dp_match_1, 1):
#             then:
#                 block _dp_bb_0:
#                     one()
#                     return __dp_NONE
#             else:
#                 block _dp_bb_1:
#                     other()
#                     return __dp_NONE

# generator_yield


def gen():
    yield 1


# ==

# generator gen():
#     function_id: 0
#     block _dp_bb_start:
#         yield 1
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "gen", __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# yield_from


def gen():
    yield from it


# ==

# generator gen():
#     function_id: 0
#     block _dp_bb_start:
#         yield from it
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "gen", __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_7, _dp_try_exc_1, _dp_try_abrupt_kind_2]
#     block _dp_bb_start:
#         _dp_tmp_4 = Suppress()
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_4)
#         __dp_contextmanager_enter(_dp_tmp_4)
#         _dp_with_ok_2 = True
#         try_jump:
#             body_label: _dp_bb_11
#             except_label: _dp_bb_10
#         block _dp_bb_10:
#             exc_param: _dp_try_exc_1
#             params: [_dp_try_exc_1:Exception]
#             if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#                 then:
#                     block _dp_bb_8:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         _dp_with_ok_2 = False
#                         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#                         jump _dp_bb_3
#                         block _dp_bb_3:
#                             jump _dp_bb_2(Fallthrough, None)
#                             block _dp_bb_2:
#                                 exc_param: _dp_try_exc_7
#                                 params: [_dp_try_abrupt_kind_2:AbruptKind, _dp_try_abrupt_payload_3:AbruptPayload, _dp_try_exc_7:Exception]
#                                 if_term _dp_with_ok_2:
#                                     then:
#                                         block _dp_bb_1:
#                                             exc_param: _dp_try_exc_7
#                                             params: [_dp_try_exc_7:Exception]
#                                             __dp_contextmanager_exit(_dp_with_exit_1, None)
#                                             jump _dp_bb_0
#                                     else:
#                                         jump _dp_bb_0
#                                 block _dp_bb_0:
#                                     exc_param: _dp_try_exc_7
#                                     params: [_dp_try_exc_7:Exception]
#                                     _dp_with_exit_1 = None
#                                     _dp_tmp_4 = None
#                                     jump _dp_bb_7
#                                     block _dp_bb_7:
#                                         branch_table _dp_try_abrupt_kind_2 -> [_dp_bb_12, _dp_bb_5, _dp_bb_6] default _dp_bb_12
#                                         block _dp_bb_12:
#                                             return __dp_NONE
#                                         block _dp_bb_5:
#                                             return _dp_try_abrupt_payload_3
#                                         block _dp_bb_6:
#                                             raise _dp_try_abrupt_payload_3
#                 else:
#                     block _dp_bb_9:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         raise
#         block _dp_bb_11:
#             raise RuntimeError("boom")
#     block _dp_bb_4:
#         exc_param: _dp_try_exc_7
#         params: [_dp_try_exc_7:Exception]
#         jump _dp_bb_2(Exception, _dp_try_exc_7)

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# function outer.<locals>.inner():
#     function_id: 0
#     entry_liveins: [_dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     block _dp_bb_start:
#         return __dp_load_cell(_dp_cell_x)

# function outer():
#     function_id: 1
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block _dp_bb_start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         inner = __dp_make_function(0, __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), None)
#         return inner()

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_start:
#         __dp_store_global(globals(), "outer", __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
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
#     block _dp_bb_start:
#         total = a + b
#         if_term __dp_gt(total, 5):
#             then:
#                 block _dp_bb_0:
#                     return a
#             else:
#                 block _dp_bb_1:
#                     return b

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "choose", __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
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
#     entry_liveins: [_dp_cell_x]
#     freevars: [x->_dp_cell_x@inherited]
#     block _dp_bb_start:
#         __dp_store_cell(_dp_cell_x, 2)
#         return __dp_load_cell(_dp_cell_x)

# function outer():
#     function_id: 1
#     local_cell_slots: [_dp_cell_x]
#     cellvars: [x->_dp_cell_x@deferred]
#     block _dp_bb_start:
#         _dp_cell_x = __dp_make_cell()
#         __dp_store_cell(_dp_cell_x, 5)
#         inner = __dp_make_function(0, __dp_tuple(__dp_tuple("_dp_cell_x", _dp_cell_x)), __dp_tuple(), __dp_globals(), None)
#         return inner()

# function _dp_module_init():
#     function_id: 2
#     block _dp_bb_start:
#         __dp_store_global(globals(), "outer", __dp_make_function(1, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# function _dp_module_init():
#     function_id: 0
#     entry_liveins: [_dp_try_exc_1]
#     block _dp_bb_start:
#         try_jump:
#             body_label: _dp_bb_3
#             except_label: _dp_bb_2
#         block _dp_bb_2:
#             exc_param: _dp_try_exc_1
#             params: [_dp_try_exc_1:Exception]
#             if_term __dp_exception_matches(__dp_current_exception(), Exception):
#                 then:
#                     block _dp_bb_0:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         print(2)
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_1:
#                         exc_param: _dp_try_exc_1
#                         params: [_dp_try_exc_1:Exception]
#                         raise
#         block _dp_bb_3:
#             print(1)
#             return __dp_NONE

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
#     entry_liveins: [a, _dp_try_exc_7]
#     block _dp_bb_start:
#         _dp_iter_1 = __dp_iter(a)
#         jump _dp_bb_7
#         block _dp_bb_7:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         print("finsihed")
#                         return __dp_NONE
#                 else:
#                     block _dp_bb_6:
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_5
#                         block _dp_bb_5:
#                             try_jump:
#                                 body_label: _dp_bb_4
#                                 except_label: _dp_bb_3
#                             block _dp_bb_3:
#                                 exc_param: _dp_try_exc_7
#                                 params: [_dp_try_exc_7:Exception]
#                                 if_term __dp_exception_matches(__dp_current_exception(), Exception):
#                                     then:
#                                         block _dp_bb_1:
#                                             exc_param: _dp_try_exc_7
#                                             params: [_dp_try_exc_7:Exception]
#                                             print("oops")
#                                             jump _dp_bb_7
#                                     else:
#                                         block _dp_bb_2:
#                                             exc_param: _dp_try_exc_7
#                                             params: [_dp_try_exc_7:Exception]
#                                             raise
#                             block _dp_bb_4:
#                                 j = i + 1
#                                 yield j
#                                 jump _dp_bb_7

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb_start:
#         __dp_store_global(globals(), "complicated", __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE
