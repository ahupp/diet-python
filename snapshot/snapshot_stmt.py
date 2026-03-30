# import_simple

import a

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         a = __dp_import_("a", __spec__)
#         return __dp_NONE

# import_dotted_alias

import a.b as c

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         c = __dp_import_attr(__dp_import_("a.b", __spec__), "b")
#         return __dp_NONE

# import_from_alias

from pkg.mod import name as alias

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
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
#     block bb1:
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         f = dec(__dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None))
#         return __dp_NONE

# assign_attr

obj.x = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return __dp_NONE

# assign_subscript

obj[i] = v

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)
#         return __dp_NONE

# assign_tuple_unpack

a, b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#         a = __dp_getitem(_dp_tmp_1, 0)
#         b = __dp_getitem(_dp_tmp_1, 1)
#         del _dp_tmp_1
#         return __dp_NONE

# assign_star_unpack

a, *b = it

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#         a = __dp_getitem(_dp_tmp_1, 0)
#         b = __dp_list(__dp_getitem(_dp_tmp_1, 1))
#         del _dp_tmp_1
#         return __dp_NONE

# assign_multi_targets

a = b = f()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         _dp_tmp_1 = f()
#         a = _dp_tmp_1
#         b = _dp_tmp_1
#         return __dp_NONE

# ann_assign_simple

x: int = 1

# ==

# function __annotate__(_dp_format, _dp):
#     function_id: 0
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

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         x = 1
#         __annotate__ = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(runtime), __dp_globals(), None)
#         return __dp_NONE

# ann_assign_attr

obj.x: int = 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)
#         return __dp_NONE

# aug_assign_attr

obj.x += 1

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))
#         return __dp_NONE

# delete_mixed

del obj.x, obj[i], x

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         __dp_delattr(obj, "x")
#         __dp_delitem(obj, i)
#         del x
#         return __dp_NONE

# assert_no_msg

assert cond

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term __debug__:
#             then:
#                 block bb2:
#                     if_term not cond:
#                         then:
#                             block bb3:
#                                 raise __dp_AssertionError
#                         else:
#                             jump bb0
#             else:
#                 jump bb0
#         block bb0:
#             return __dp_NONE

# assert_with_msg

assert cond, "oops"

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term __debug__:
#             then:
#                 block bb2:
#                     if_term not cond:
#                         then:
#                             block bb3:
#                                 raise __dp_AssertionError("oops")
#                         else:
#                             jump bb0
#             else:
#                 jump bb0
#         block bb0:
#             return __dp_NONE

# raise_from

raise E from cause

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
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
#     block bb3:
#         _dp_iter_0_0 = __dp_iter(it)
#         jump bb1
#         block bb1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         done()
#                         return __dp_NONE
#                 else:
#                     block bb2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             body()
#                             jump bb1

# while_else

while cond:
    body()
else:
    done()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term cond:
#             then:
#                 block bb3:
#                     body()
#                     jump bb1
#             else:
#                 block bb2:
#                     done()
#                     return __dp_NONE

# with_as

with cm as x:
    body()

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb4:
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
#         x = __dp_contextmanager_enter(cm)
#         _dp_with_ok_2 = True
#         jump bb13
#         block bb13:
#             body()
#             jump bb8
#             block bb8:
#                 jump bb5(AbruptKind(Fallthrough), None)
#                 block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#                     exc_param: _dp_try_exc_0_0
#                     if_term _dp_with_ok_2:
#                         then:
#                             block bb7(_dp_try_exc_0_0: Exception):
#                                 exc_param: _dp_try_exc_0_0
#                                 __dp_contextmanager_exit(_dp_with_exit_1, None)
#                                 jump bb6
#                         else:
#                             jump bb6
#                     block bb6(_dp_try_exc_0_0: Exception):
#                         exc_param: _dp_try_exc_0_0
#                         _dp_with_exit_1 = None
#                         jump bb1
#                         block bb1:
#                             branch_table _dp_try_abrupt_kind_0_1 -> [bb0, bb2, bb3] default bb0
#                             block bb0:
#                                 return __dp_NONE
#                             block bb2:
#                                 return _dp_try_abrupt_payload_0_2
#                             block bb3:
#                                 raise _dp_try_abrupt_payload_0_2
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term __dp_exception_matches(_dp_try_exc_0_0, BaseException):
#             then:
#                 jump bb11
#             else:
#                 jump bb12
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_0_0))
#         jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise _dp_try_exc_0_0

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# function inner():
#     function_id: 0
#     block bb1:
#         value = 1
#         return value

# function _dp_module_init():
#     function_id: 1
#     block bb1:
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
#     block bb3:
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump bb1
#         block bb1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         return _dp_tmp_1
#                 else:
#                     block bb2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             _dp_tmp_1.append(x)
#                             jump bb1

# function _dp_setcomp_6(_dp_iter_5):
#     function_id: 1
#     display_name: <setcomp>
#     block bb3:
#         _dp_tmp_4 = set()
#         _dp_iter_1_0 = __dp_iter(_dp_iter_5)
#         jump bb1
#         block bb1:
#             _dp_tmp_1_1 = __dp_next_or_sentinel(_dp_iter_1_0)
#             if_term __dp_is_(_dp_tmp_1_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         return _dp_tmp_4
#                 else:
#                     block bb2:
#                         x = _dp_tmp_1_1
#                         _dp_tmp_1_1 = None
#                         jump bb5
#                         block bb5:
#                             _dp_tmp_4.add(x)
#                             jump bb1

# function _dp_dictcomp_9(_dp_iter_8):
#     function_id: 2
#     display_name: <dictcomp>
#     block bb3:
#         _dp_tmp_7 = {}
#         _dp_iter_2_0 = __dp_iter(_dp_iter_8)
#         jump bb1
#         block bb1:
#             _dp_tmp_2_1 = __dp_next_or_sentinel(_dp_iter_2_0)
#             if_term __dp_is_(_dp_tmp_2_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         return _dp_tmp_7
#                 else:
#                     block bb2:
#                         _dp_tmp_2_2 = __dp_unpack(_dp_tmp_2_1, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_2_2, 0)
#                         v = __dp_getitem(_dp_tmp_2_2, 1)
#                         del _dp_tmp_2_2
#                         _dp_tmp_2_1 = None
#                         jump bb5
#                         block bb5:
#                             __dp_setitem(_dp_tmp_7, k, v)
#                             jump bb1

# function _dp_module_init():
#     function_id: 3
#     block bb1:
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
#     block bb3:
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump bb1
#         block bb1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         return _dp_tmp_1
#                 else:
#                     block bb2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             if_term __dp_gt(x, 0):
#                                 then:
#                                     block bb6:
#                                         _dp_tmp_1.append(x)
#                                         jump bb1
#                                 else:
#                                     jump bb1

# function f():
#     function_id: 1
#     block bb1:
#         _dp_listcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return _dp_listcomp_3(it)

# function _dp_module_init():
#     function_id: 2
#     block bb1:
#         f = __dp_make_function(1, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# function C._dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block bb3:
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump bb1
#         block bb1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         return _dp_tmp_1
#                 else:
#                     block bb2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             _dp_tmp_1.append(x)
#                             jump bb1

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
#     function_id: 1
#     block bb1:
#         _dp_classcell = _dp_classcell_arg
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#         __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#         _dp_listcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         xs = _dp_listcomp_3(it)
#         return __dp_NONE

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict):
#     function_id: 2
#     block bb1:
#         _dp_class_ns = _dp_class_ns_outer
#         return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init():
#     function_id: 3
#     block bb1:
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
#     block bb4:
#         _dp_with_exit_4 = __dp_contextmanager_get_exit(a)
#         x = __dp_contextmanager_enter(a)
#         _dp_with_ok_5 = True
#         jump bb16
#         block bb16:
#             _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
#             y = __dp_contextmanager_enter(b)
#             _dp_with_ok_2 = True
#             jump bb25
#             block bb25:
#                 body()
#                 jump bb20
#                 block bb20:
#                     jump bb17(AbruptKind(Fallthrough), None)
#                     block bb17(_dp_try_exc_0_3: Exception, _dp_try_abrupt_kind_0_4: AbruptKind, _dp_try_abrupt_payload_0_5: AbruptPayload):
#                         exc_param: _dp_try_exc_0_3
#                         if_term _dp_with_ok_2:
#                             then:
#                                 block bb19(_dp_try_exc_0_3: Exception):
#                                     exc_param: _dp_try_exc_0_3
#                                     __dp_contextmanager_exit(_dp_with_exit_1, None)
#                                     jump bb18
#                             else:
#                                 jump bb18
#                         block bb18(_dp_try_exc_0_3: Exception):
#                             exc_param: _dp_try_exc_0_3
#                             _dp_with_exit_1 = None
#                             jump bb13
#                             block bb13:
#                                 branch_table _dp_try_abrupt_kind_0_4 -> [bb8, bb14, bb15] default bb8
#                                 block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#                                     exc_param: _dp_try_exc_0_0
#                                     if_term _dp_with_ok_5:
#                                         then:
#                                             block bb7(_dp_try_exc_0_0: Exception):
#                                                 exc_param: _dp_try_exc_0_0
#                                                 __dp_contextmanager_exit(_dp_with_exit_4, None)
#                                                 jump bb6
#                                         else:
#                                             jump bb6
#                                     block bb6(_dp_try_exc_0_0: Exception):
#                                         exc_param: _dp_try_exc_0_0
#                                         _dp_with_exit_4 = None
#                                         jump bb1
#                                         block bb1:
#                                             branch_table _dp_try_abrupt_kind_0_1 -> [bb0, bb2, bb3] default bb0
#                                             block bb0:
#                                                 return __dp_NONE
#                                             block bb2:
#                                                 return _dp_try_abrupt_payload_0_2
#                                             block bb3:
#                                                 raise _dp_try_abrupt_payload_0_2
#                                 block bb8:
#                                     jump bb5(AbruptKind(Fallthrough), None)
#                                 block bb14:
#                                     _dp_try_abrupt_payload_0_2 = _dp_try_abrupt_payload_0_5
#                                     jump bb5(AbruptKind(Return), Name("_dp_try_abrupt_payload_0_2"))
#                                 block bb15:
#                                     raise _dp_try_abrupt_payload_0_5
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term __dp_exception_matches(_dp_try_exc_0_0, BaseException):
#             then:
#                 jump bb11
#             else:
#                 jump bb12
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         _dp_with_ok_5 = False
#         __dp_contextmanager_exit(_dp_with_exit_4, __dp_exc_info_from_exception(_dp_try_exc_0_0))
#         jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise _dp_try_exc_0_0
#     block bb21(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         jump bb17(AbruptKind(Exception), Name("_dp_try_exc_0_3"))
#     block bb22(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         if_term __dp_exception_matches(_dp_try_exc_0_3, BaseException):
#             then:
#                 jump bb23
#             else:
#                 jump bb24
#     block bb23(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_0_3))
#         jump bb20
#     block bb24(_dp_try_exc_0_3: Exception):
#         exc_param: _dp_try_exc_0_3
#         raise _dp_try_exc_0_3

# async_for


async def run():
    async for x in ait:
        body()


# ==

# coroutine run():
#     function_id: 0
#     block bb3:
#         _dp_iter_0_0 = __dp_aiter(ait)
#         jump bb1
#         block bb1:
#             _dp_tmp_0_1 = await __dp_anext_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb0:
#                         return __dp_NONE
#                 else:
#                     block bb2:
#                         x = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb4
#                         block bb4:
#                             body()
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         run = __dp_make_function(0, "coroutine", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# async_with


async def run():
    async with cm as x:
        body()


# ==

# coroutine run():
#     function_id: 0
#     block bb4:
#         _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
#         x = await __dp_asynccontextmanager_aenter(cm)
#         _dp_with_ok_2 = True
#         jump bb14
#         block bb14:
#             body()
#             jump bb8
#             block bb8:
#                 jump bb5(AbruptKind(Fallthrough), None)
#                 block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#                     exc_param: _dp_try_exc_0_0
#                     if_term _dp_with_ok_2:
#                         then:
#                             block bb7(_dp_try_exc_0_0: Exception):
#                                 exc_param: _dp_try_exc_0_0
#                                 await __dp_asynccontextmanager_exit(_dp_with_exit_1, None)
#                                 jump bb6
#                         else:
#                             jump bb6
#                     block bb6(_dp_try_exc_0_0: Exception):
#                         exc_param: _dp_try_exc_0_0
#                         _dp_with_exit_1 = None
#                         jump bb1
#                         block bb1:
#                             branch_table _dp_try_abrupt_kind_0_1 -> [bb0, bb2, bb3] default bb0
#                             block bb0:
#                                 return __dp_NONE
#                             block bb2:
#                                 return _dp_try_abrupt_payload_0_2
#                             block bb3:
#                                 raise _dp_try_abrupt_payload_0_2
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term __dp_exception_matches(__dp_current_exception(), BaseException):
#             then:
#                 jump bb11
#             else:
#                 jump bb13
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         _dp_with_ok_2 = False
#         _dp_with_reraise_3 = await __dp_asynccontextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#         if_term __dp_is_not(_dp_with_reraise_3, None):
#             then:
#                 jump bb12
#             else:
#                 jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise _dp_with_reraise_3
#     block bb13(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise

# function _dp_module_init():
#     function_id: 1
#     block bb1:
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
#     block bb1:
#         _dp_match_1 = value
#         if_term __dp_eq(_dp_match_1, 1):
#             then:
#                 block bb2:
#                     one()
#                     return __dp_NONE
#             else:
#                 block bb3:
#                     other()
#                     return __dp_NONE

# generator_yield


def gen():
    yield 1


# ==

# generator gen():
#     function_id: 0
#     block bb1:
#         yield 1
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         gen = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# yield_from


def gen():
    yield from it


# ==

# generator gen():
#     function_id: 0
#     block bb1:
#         yield from it
#         return __dp_NONE

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         gen = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb4:
#         _dp_tmp_4 = Suppress()
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_4)
#         __dp_contextmanager_enter(_dp_tmp_4)
#         _dp_with_ok_2 = True
#         jump bb13
#         block bb13:
#             raise RuntimeError("boom")
#     block bb0:
#         return __dp_NONE
#     block bb1:
#         branch_table _dp_try_abrupt_kind_0_1 -> [bb0, bb2, bb3] default bb0
#     block bb2:
#         return _dp_try_abrupt_payload_0_2
#     block bb3:
#         raise _dp_try_abrupt_payload_0_2
#     block bb5(_dp_try_exc_0_0: Exception, _dp_try_abrupt_kind_0_1: AbruptKind, _dp_try_abrupt_payload_0_2: AbruptPayload):
#         exc_param: _dp_try_exc_0_0
#         if_term _dp_with_ok_2:
#             then:
#                 jump bb7
#             else:
#                 jump bb6
#     block bb6(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         _dp_with_exit_1 = None
#         _dp_tmp_4 = None
#         jump bb1
#     block bb7(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         __dp_contextmanager_exit(_dp_with_exit_1, None)
#         jump bb6
#     block bb8:
#         jump bb5(AbruptKind(Fallthrough), None)
#     block bb9(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         jump bb5(AbruptKind(Exception), Name("_dp_try_exc_0_0"))
#     block bb10(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term __dp_exception_matches(_dp_try_exc_0_0, BaseException):
#             then:
#                 jump bb11
#             else:
#                 jump bb12
#     block bb11(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_0_0))
#         jump bb8
#     block bb12(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise _dp_try_exc_0_0

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# function outer.<locals>.inner():
#     function_id: 0
#     block bb1:
#         return x

# function outer():
#     function_id: 1
#     block bb1:
#         x = 5
#         inner = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return inner()

# function _dp_module_init():
#     function_id: 2
#     block bb1:
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
#     block bb1:
#         total = a + b
#         if_term __dp_gt(total, 5):
#             then:
#                 block bb2:
#                     return a
#             else:
#                 block bb3:
#                     return b

# function _dp_module_init():
#     function_id: 1
#     block bb1:
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
#     block bb1:
#         x = 2
#         return x

# function outer():
#     function_id: 1
#     block bb1:
#         x = 5
#         inner = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return inner()

# function _dp_module_init():
#     function_id: 2
#     block bb1:
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
#     block bb1:
#         jump bb5
#         block bb5:
#             print(1)
#             return __dp_NONE
#     block bb2(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         if_term __dp_exception_matches(_dp_try_exc_0_0, Exception):
#             then:
#                 jump bb3
#             else:
#                 jump bb4
#     block bb3(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         print(2)
#         return __dp_NONE
#     block bb4(_dp_try_exc_0_0: Exception):
#         exc_param: _dp_try_exc_0_0
#         raise _dp_try_exc_0_0

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
#     block bb3:
#         _dp_iter_0_0 = __dp_iter(a)
#         jump bb1
#         block bb1:
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, runtime.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         print("finsihed")
#                         return __dp_NONE
#                 else:
#                     block bb2:
#                         i = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             jump bb9
#                             block bb9:
#                                 j = i + 1
#                                 yield j
#                                 jump bb1
#     block bb6(_dp_try_exc_0_2: Exception):
#         exc_param: _dp_try_exc_0_2
#         if_term __dp_exception_matches(__dp_current_exception(), Exception):
#             then:
#                 jump bb7
#             else:
#                 jump bb8
#     block bb7(_dp_try_exc_0_2: Exception):
#         exc_param: _dp_try_exc_0_2
#         print("oops")
#         jump bb1
#     block bb8(_dp_try_exc_0_2: Exception):
#         exc_param: _dp_try_exc_0_2
#         raise

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         complicated = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         return __dp_NONE
