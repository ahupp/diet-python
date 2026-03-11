# import_simple

import a

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "a", __dp_import_("a", __spec__))

# import_dotted_alias

import a.b as c

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "c", __dp_import_attr(__dp_import_("a.b", __spec__), "b"))

# import_from_alias

from pkg.mod import name as alias

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_import_1 = __dp_import_("pkg.mod", __spec__, __dp_list(__dp_tuple("name")))
#     __dp_store_global(globals(), "alias", __dp_import_attr(_dp_import_1, "name"))

# decorator_function


@dec
def f():
    pass


# ==

# module_init: _dp_module_init

# function f() [kind=function, bind=f, target=module_global, qualname=f]
#     pass

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def f(): ...

# assign_attr

obj.x = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)

# assign_subscript

obj[i] = v

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_setitem(__dp_load_deleted_name("obj", obj), i, v)

# assign_tuple_unpack

a, b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, True))
#     __dp_store_global(globals(), "a", __dp_getitem(_dp_tmp_1, 0))
#     __dp_store_global(globals(), "b", __dp_getitem(_dp_tmp_1, 1))
#     del _dp_tmp_1

# assign_star_unpack

a, *b = it

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_tmp_1 = __dp_unpack(it, __dp_tuple(True, False))
#     __dp_store_global(globals(), "a", __dp_getitem(_dp_tmp_1, 0))
#     __dp_store_global(globals(), "b", __dp_list(__dp_getitem(_dp_tmp_1, 1)))
#     del _dp_tmp_1

# assign_multi_targets

a = b = f()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_tmp_1 = f()
#     __dp_store_global(globals(), "a", _dp_tmp_1)
#     __dp_store_global(globals(), "b", _dp_tmp_1)

# ann_assign_simple

x: int = 1

# ==

# module_init: _dp_module_init

# function __annotate__(_dp_format, _dp = __dp__) [kind=function, bind=__annotate__, target=module_global, qualname=__annotate__]
#     if _dp.eq(_dp_format, 4):
#         return _dp.dict(__dp_tuple(("x", "int")))
#     if _dp.gt(_dp_format, 2):
#         raise _dp.builtins.NotImplementedError
#     return _dp.dict(__dp_tuple(("x", int)))

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", 1)
#     def __annotate__(_dp_format, _dp = __dp__): ...

# ann_assign_attr

obj.x: int = 1

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_setattr(__dp_load_deleted_name("obj", obj), "x", 1)

# aug_assign_attr

obj.x += 1

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_setattr(__dp_load_deleted_name("obj", obj), "x", __dp_iadd(obj.x, 1))

# delete_mixed

del obj.x, obj[i], x

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_delattr(obj, "x")
#     __dp_delitem(obj, i)
#     __dp_delitem(globals(), "x")

# assert_no_msg

assert cond

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     if __debug__:
#         if __dp_not_(cond):
#             raise __dp_AssertionError

# assert_with_msg

assert cond, "oops"

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     if __debug__:
#         if __dp_not_(cond):
#             raise __dp_AssertionError("oops")

# raise_from

raise E from cause

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     raise __dp_raise_from(E, cause)

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
#     try:
#         f()
#     except:
#         if __dp_exception_matches(__dp_current_exception(), E):
#             __dp_store_global(globals(), "e", __dp_current_exception())
#             try:
#                 g(__dp_load_global(globals(), "e"))
#             else:
#                 pass
#             finally:
#                 __dp_delitem_quietly(globals(), "e")
#         else:
#             h()
#     else:
#         pass
#     finally:
#         pass

# for_else

for x in it:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     pass
#     _dp_iter_1 = __dp_iter(it)
#     block for_fetch_1:
#         _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#         if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#             jump for_else_4
#         else:
#             x = _dp_tmp_2
#             _dp_tmp_2 = None
#             jump for_body_2
#     block for_body_2:
#         __dp_store_global(globals(), "x", x)
#         body()
#         jump for_fetch_1
#     block for_else_4:
#         done()
#     pass

# while_else

while cond:
    body()
else:
    done()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     pass
#     block while_test_0:
#         if cond:
#             jump while_body_1
#         else:
#             jump while_else_3
#     block while_body_1:
#         body()
#         jump while_test_0
#     block while_else_3:
#         done()
#     pass

# with_as

with cm as x:
    body()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
#     x = __dp_contextmanager_enter(cm)
#     _dp_with_ok_2 = True
#     try:
#         body()
#     except BaseException:
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#     else:
#         pass
#     finally:
#         if _dp_with_ok_2:
#             __dp_contextmanager_exit(_dp_with_exit_1, None)
#         _dp_with_exit_1 = None

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==

# module_init: _dp_module_init

# function inner() [kind=function, bind=inner, target=module_global, qualname=inner]
#     value = 1
#     return value

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def inner(): ...

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=_dp_listcomp_3]
#     _dp_tmp_1 = __dp_list(__dp_tuple())
#     _dp_iter_10 = __dp_iter(_dp_iter_2)
#     block for_fetch_1:
#         _dp_tmp_11 = __dp_next_or_sentinel(_dp_iter_10)
#         if __dp_is_(_dp_tmp_11, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             x = _dp_tmp_11
#             _dp_tmp_11 = None
#             jump for_body_2
#     block for_body_2:
#         _dp_tmp_1.append(x)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_1

# function _dp_setcomp_6(_dp_iter_5) [kind=function, bind=_dp_setcomp_6, target=local, qualname=_dp_setcomp_6]
#     _dp_tmp_4 = set()
#     _dp_iter_12 = __dp_iter(_dp_iter_5)
#     block for_fetch_1:
#         _dp_tmp_13 = __dp_next_or_sentinel(_dp_iter_12)
#         if __dp_is_(_dp_tmp_13, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             x = _dp_tmp_13
#             _dp_tmp_13 = None
#             jump for_body_2
#     block for_body_2:
#         _dp_tmp_4.add(x)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_4

# function _dp_dictcomp_9(_dp_iter_8) [kind=function, bind=_dp_dictcomp_9, target=local, qualname=_dp_dictcomp_9]
#     _dp_tmp_7 = __dp_dict()
#     _dp_iter_14 = __dp_iter(_dp_iter_8)
#     block for_fetch_1:
#         _dp_tmp_15 = __dp_next_or_sentinel(_dp_iter_14)
#         if __dp_is_(_dp_tmp_15, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             _dp_tmp_16 = __dp_unpack(_dp_tmp_15, __dp_tuple(True, True))
#             k = __dp_getitem(_dp_tmp_16, 0)
#             v = __dp_getitem(_dp_tmp_16, 1)
#             del _dp_tmp_16
#             _dp_tmp_15 = None
#             jump for_body_2
#     block for_body_2:
#         __dp_setitem(_dp_tmp_7, k, v)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_7

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_listcomp_3(_dp_iter_2): ...
#     __dp_store_global(globals(), "xs", _dp_listcomp_3(it))
#     def _dp_setcomp_6(_dp_iter_5): ...
#     __dp_store_global(globals(), "ys", _dp_setcomp_6(it))
#     def _dp_dictcomp_9(_dp_iter_8): ...
#     __dp_store_global(globals(), "zs", _dp_dictcomp_9(items))

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=f.<locals>._dp_listcomp_3]
#     _dp_tmp_1 = __dp_list(__dp_tuple())
#     _dp_iter_4 = __dp_iter(_dp_iter_2)
#     _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
#     if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
#         jump for_after_3
#     else:
#         x = _dp_tmp_5
#         _dp_tmp_5 = None
#         jump for_body_2
#     block for_body_2:
#         if __dp_gt(x, 0):
#             _dp_tmp_1.append(x)
#     block for_after_3:
#         return _dp_tmp_1

# function f() [kind=function, bind=f, target=module_global, qualname=f]
#     def _dp_listcomp_3(_dp_iter_2): ...
#     return _dp_listcomp_3(it)

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def f(): ...

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=C._dp_listcomp_3]
#     _dp_tmp_1 = __dp_list(__dp_tuple())
#     _dp_iter_4 = __dp_iter(_dp_iter_2)
#     block for_fetch_1:
#         _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
#         if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             x = _dp_tmp_5
#             _dp_tmp_5 = None
#             jump for_body_2
#     block for_body_2:
#         _dp_tmp_1.append(x)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_1

# function _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg) [kind=function, bind=_dp_class_ns_C, target=local, qualname=_dp_class_ns_C]
#     _dp_classcell = _dp_classcell_arg
#     __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__module__", __name__)
#     __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "__qualname__", "C")
#     def _dp_listcomp_3(_dp_iter_2): ...
#     __dp_setitem(__dp_load_deleted_name("_dp_class_ns", _dp_class_ns), "xs", _dp_listcomp_3(__dp_class_lookup_global(_dp_class_ns, "it", globals())))

# function _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None) [kind=function, bind=_dp_define_class_C, target=local, qualname=_dp_define_class_C]
#     _dp_class_ns = _dp_class_ns_outer
#     return __dp_create_class("C", _dp_class_ns_fn, __dp_tuple(), _dp_prepare_dict, False, 3, ())

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg): ...
#     def _dp_define_class_C(_dp_class_ns_fn, _dp_class_ns_outer, _dp_prepare_dict = None): ...
#     __dp_store_global(globals(), "C", _dp_define_class_C(_dp_class_ns_C, globals()))

# with_multi

with a as x, b as y:
    body()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_with_exit_4 = __dp_contextmanager_get_exit(a)
#     x = __dp_contextmanager_enter(a)
#     _dp_with_ok_5 = True
#     try:
#         _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
#         y = __dp_contextmanager_enter(b)
#         _dp_with_ok_2 = True
#         try:
#             body()
#         except BaseException:
#             _dp_with_ok_2 = False
#             __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#         else:
#             pass
#         finally:
#             if _dp_with_ok_2:
#                 __dp_contextmanager_exit(_dp_with_exit_1, None)
#             _dp_with_exit_1 = None
#     except BaseException:
#         _dp_with_ok_5 = False
#         __dp_contextmanager_exit(_dp_with_exit_4, __dp_exc_info())
#     else:
#         pass
#     finally:
#         if _dp_with_ok_5:
#             __dp_contextmanager_exit(_dp_with_exit_4, None)
#         _dp_with_exit_4 = None

# async_for


async def run():
    async for x in ait:
        body()


# ==

# module_init: _dp_module_init

# function run() [kind=coroutine, bind=run, target=module_global, qualname=run]
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: start
#         resume_order: [start, _dp_bb_run_11]
#         yield_sites:
#             _dp_bb_run_9 -> _dp_bb_run_11
#         done_block_label: run_done
#         invalid_block_label: run_invalid
#         uncaught_block_label: run_uncaught
#         uncaught_set_done_label: run_uncaught_set_done
#         uncaught_raise_label: run_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_15
#         dispatch_only_labels: [run_dispatch, run_dispatch_invalid, run_dispatch_send, run_dispatch_send_table, run_dispatch_send_target_0, run_dispatch_send_target_1, run_dispatch_throw, run_dispatch_throw_done, run_dispatch_throw_table, run_dispatch_throw_target_0, run_dispatch_throw_target_1, run_dispatch_throw_unstarted]
#         throw_passthrough_labels: [run_dispatch_throw_done, run_dispatch_throw_unstarted, run_uncaught_raise, run_uncaught_set_done]
#     block run_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block run_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump run_uncaught_set_done
#     else:
#         jump run_uncaught_raise
#     block run_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_15)
#         jump run_uncaught_raise
#     block run_uncaught_raise:
#         raise _dp_uncaught_exc_15
#     block start:
#         if False:
#             jump _dp_bb_run_9
#             block _dp_bb_run_10:
#                 raise _dp_resume_exc
#             block _dp_bb_run_11:
#                 if __dp_is_not(_dp_resume_exc, None):
#                     jump _dp_bb_run_10
#                 else:
#                     jump _dp_bb_run_end_8
#             block _dp_bb_run_9:
#                 return __dp_NONE
#             block _dp_bb_run_end_8:
#                 return
#         else:
#             return
#     _dp_iter_1 = __dp_aiter(ait)
#     jump for_fetch_2
#     block for_fetch_2:
#         _dp_tmp_2 = await __dp_anext_or_sentinel(_dp_iter_1)
#         if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#             jump for_after_4
#             return
#         else:
#             x = _dp_tmp_2
#             _dp_tmp_2 = None
#             jump for_body_3
#             return
#     block for_body_3:
#         body()
#         jump for_fetch_2
#     block for_after_4:
#         jump _dp_bb_run_end_7
#     block _dp_bb_run_end_7:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block run_dispatch_throw_done:
#         raise _dp_resume_exc
#     block run_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block run_dispatch_send_target_0:
#         jump start
#     block run_dispatch_throw_target_0:
#         jump run_dispatch_throw_unstarted
#     block run_dispatch_send_target_1:
#         jump _dp_bb_run_11
#     block run_dispatch_throw_target_1:
#         jump _dp_bb_run_11
#     block run_dispatch_invalid:
#         jump run_invalid
#     block run_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_dispatch_send_target_0, run_dispatch_send_target_1] default run_dispatch_invalid
#     block run_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_dispatch_throw_target_0, run_dispatch_throw_target_1] default run_dispatch_invalid
#     block run_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump run_done
#         else:
#             jump run_dispatch_send_table
#     block run_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump run_dispatch_throw_done
#         else:
#             jump run_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump run_dispatch_send
#     else:
#         jump run_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def run(): ...

# async_with


async def run():
    async with cm as x:
        body()


# ==

# module_init: _dp_module_init

# function run() [kind=coroutine, bind=run, target=module_global, qualname=run]
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: _dp_bb_run_80
#         resume_order: [_dp_bb_run_80, _dp_bb_run_11, _dp_bb_run_36, _dp_bb_run_67, _dp_bb_run_99]
#         yield_sites:
#             _dp_bb_run_9 -> _dp_bb_run_11
#             _dp_bb_run_47 -> _dp_bb_run_36
#             _dp_bb_run_78 -> _dp_bb_run_67
#             _dp_bb_run_110 -> _dp_bb_run_99
#         done_block_label: run_done
#         invalid_block_label: run_invalid
#         uncaught_block_label: run_uncaught
#         uncaught_set_done_label: run_uncaught_set_done
#         uncaught_raise_label: run_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_111
#         dispatch_only_labels: [run_dispatch, run_dispatch_invalid, run_dispatch_send, run_dispatch_send_table, run_dispatch_send_target_0, run_dispatch_send_target_1, run_dispatch_send_target_2, run_dispatch_send_target_3, run_dispatch_send_target_4, run_dispatch_throw, run_dispatch_throw_done, run_dispatch_throw_table, run_dispatch_throw_target_0, run_dispatch_throw_target_1, run_dispatch_throw_target_2, run_dispatch_throw_target_3, run_dispatch_throw_target_4, run_dispatch_throw_unstarted]
#         throw_passthrough_labels: [run_dispatch_throw_done, run_dispatch_throw_unstarted, run_uncaught_raise, run_uncaught_set_done]
#     block run_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block run_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump run_uncaught_set_done
#     else:
#         jump run_uncaught_raise
#     block run_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_111)
#         jump run_uncaught_raise
#     block run_uncaught_raise:
#         raise _dp_uncaught_exc_111
#     block _dp_bb_run_80:
#         _dp_with_ok_2 = True
#         try:
#             body()
#             jump _dp_bb_run_end_13
#             block _dp_bb_run_end_13:
#                 return
#         except BaseException:
#             block _dp_bb_run_17:
#                 if not _dp_with_suppress_3:
#                     raise
#                     return
#                 else:
#                     return
#             jump _dp_bb_run_19
#             block _dp_bb_run_29:
#                 _dp_yield_from_iter_20 = iter(__dp_await_iter(__dp_asynccontextmanager_aexit(_dp_with_exit_1, __dp_exc_info())))
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_20)
#                 legacy_try_jump:
#                     body_label: _dp_bb_run_30
#                     except_label: _dp_bb_run_31
#                     except_exc_name: _dp_try_exc_24
#                     body_region_labels: [_dp_bb_run_30]
#                     except_region_labels: [_dp_bb_run_31, _dp_bb_run_32, _dp_bb_run_33]
#             block _dp_bb_run_30:
#                 _dp_yield_from_y_21 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#                 jump _dp_bb_run_47
#             block _dp_bb_run_31:
#                 if __dp_exception_matches(_dp_try_exc_24, StopIteration):
#                     jump _dp_bb_run_32
#                 else:
#                     jump _dp_bb_run_33
#             block _dp_bb_run_32:
#                 _dp_yield_from_result_23 = _dp_try_exc_24.value
#                 jump _dp_bb_run_34
#             block _dp_bb_run_34:
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                 jump _dp_bb_run_18
#             block _dp_bb_run_33:
#                 _dp_yield_from_raise_26 = _dp_try_exc_24
#                 jump _dp_bb_run_35
#             block _dp_bb_run_47:
#                 return _dp_yield_from_y_21
#             _dp_yield_from_sent_22 = _dp_send_value
#             _dp_yield_from_exc_25 = _dp_resume_exc
#             _dp_resume_exc = None
#             if __dp_is_not(_dp_yield_from_exc_25, None):
#                 jump _dp_bb_run_37
#             else:
#                 jump _dp_bb_run_44
#             block _dp_bb_run_37:
#                 if __dp_exception_matches(_dp_yield_from_exc_25, GeneratorExit):
#                     jump _dp_bb_run_38
#                 else:
#                     jump _dp_bb_run_41
#             block _dp_bb_run_38:
#                 _dp_yield_from_close_27 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                 if __dp_is_not(_dp_yield_from_close_27, None):
#                     jump _dp_bb_run_39
#                 else:
#                     jump _dp_bb_run_40
#             block _dp_bb_run_39:
#                 _dp_yield_from_close_27()
#                 jump _dp_bb_run_40
#             block _dp_bb_run_40:
#                 _dp_yield_from_raise_26 = _dp_yield_from_exc_25
#                 jump _dp_bb_run_35
#             block _dp_bb_run_35:
#                 __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                 raise _dp_yield_from_raise_26
#             block _dp_bb_run_41:
#                 _dp_yield_from_throw_28 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                 if __dp_is_(_dp_yield_from_throw_28, None):
#                     jump _dp_bb_run_40
#                 else:
#                     jump _dp_bb_run_42
#             block _dp_bb_run_42:
#                 legacy_try_jump:
#                     body_label: _dp_bb_run_43
#                     except_label: _dp_bb_run_31
#                     except_exc_name: _dp_try_exc_24
#                     body_region_labels: [_dp_bb_run_43]
#                     except_region_labels: [_dp_bb_run_31, _dp_bb_run_32, _dp_bb_run_33]
#             block _dp_bb_run_43:
#                 _dp_yield_from_y_21 = _dp_yield_from_throw_28(_dp_yield_from_exc_25)
#                 jump _dp_bb_run_47
#             block _dp_bb_run_44:
#                 legacy_try_jump:
#                     body_label: _dp_bb_run_45
#                     except_label: _dp_bb_run_31
#                     except_exc_name: _dp_try_exc_24
#                     body_region_labels: [_dp_bb_run_45, _dp_bb_run_30, _dp_bb_run_46]
#                     except_region_labels: [_dp_bb_run_31, _dp_bb_run_32, _dp_bb_run_33]
#             block _dp_bb_run_45:
#                 if __dp_is_(_dp_yield_from_sent_22, None):
#                     jump _dp_bb_run_30
#                 else:
#                     jump _dp_bb_run_46
#             block _dp_bb_run_46:
#                 _dp_yield_from_y_21 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_22)
#                 jump _dp_bb_run_47
#             block _dp_bb_run_18:
#                 _dp_with_suppress_3 = _dp_yield_from_result_23
#                 jump _dp_bb_run_17
#             block _dp_bb_run_19:
#                 _dp_with_ok_2 = False
#                 jump _dp_bb_run_29
#             return
#         else:
#             jump _dp_bb_run_end_48
#             block _dp_bb_run_end_48:
#                 return
#         finally:
#             if _dp_with_ok_2:
#                 jump _dp_bb_run_51
#                 block _dp_bb_run_60:
#                     _dp_yield_from_iter_52 = iter(__dp_await_iter(__dp_asynccontextmanager_aexit(_dp_with_exit_1, None)))
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_52)
#                     legacy_try_jump:
#                         body_label: _dp_bb_run_61
#                         except_label: _dp_bb_run_62
#                         except_exc_name: _dp_try_exc_55
#                         body_region_labels: [_dp_bb_run_61]
#                         except_region_labels: [_dp_bb_run_62, _dp_bb_run_63, _dp_bb_run_64]
#                 block _dp_bb_run_61:
#                     _dp_yield_from_y_53 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#                     jump _dp_bb_run_78
#                 block _dp_bb_run_62:
#                     if __dp_exception_matches(_dp_try_exc_55, StopIteration):
#                         jump _dp_bb_run_63
#                     else:
#                         jump _dp_bb_run_64
#                 block _dp_bb_run_63:
#                     jump _dp_bb_run_65
#                 block _dp_bb_run_65:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     jump _dp_bb_run_end_50
#                 block _dp_bb_run_64:
#                     _dp_yield_from_raise_57 = _dp_try_exc_55
#                     jump _dp_bb_run_66
#                 block _dp_bb_run_78:
#                     return _dp_yield_from_y_53
#                 _dp_yield_from_sent_54 = _dp_send_value
#                 _dp_yield_from_exc_56 = _dp_resume_exc
#                 _dp_resume_exc = None
#                 if __dp_is_not(_dp_yield_from_exc_56, None):
#                     jump _dp_bb_run_68
#                 else:
#                     jump _dp_bb_run_75
#                 block _dp_bb_run_68:
#                     if __dp_exception_matches(_dp_yield_from_exc_56, GeneratorExit):
#                         jump _dp_bb_run_69
#                     else:
#                         jump _dp_bb_run_72
#                 block _dp_bb_run_69:
#                     _dp_yield_from_close_58 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#                     if __dp_is_not(_dp_yield_from_close_58, None):
#                         jump _dp_bb_run_70
#                     else:
#                         jump _dp_bb_run_71
#                 block _dp_bb_run_70:
#                     _dp_yield_from_close_58()
#                     jump _dp_bb_run_71
#                 block _dp_bb_run_71:
#                     _dp_yield_from_raise_57 = _dp_yield_from_exc_56
#                     jump _dp_bb_run_66
#                 block _dp_bb_run_66:
#                     __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#                     raise _dp_yield_from_raise_57
#                 block _dp_bb_run_72:
#                     _dp_yield_from_throw_59 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#                     if __dp_is_(_dp_yield_from_throw_59, None):
#                         jump _dp_bb_run_71
#                     else:
#                         jump _dp_bb_run_73
#                 block _dp_bb_run_73:
#                     legacy_try_jump:
#                         body_label: _dp_bb_run_74
#                         except_label: _dp_bb_run_62
#                         except_exc_name: _dp_try_exc_55
#                         body_region_labels: [_dp_bb_run_74]
#                         except_region_labels: [_dp_bb_run_62, _dp_bb_run_63, _dp_bb_run_64]
#                 block _dp_bb_run_74:
#                     _dp_yield_from_y_53 = _dp_yield_from_throw_59(_dp_yield_from_exc_56)
#                     jump _dp_bb_run_78
#                 block _dp_bb_run_75:
#                     legacy_try_jump:
#                         body_label: _dp_bb_run_76
#                         except_label: _dp_bb_run_62
#                         except_exc_name: _dp_try_exc_55
#                         body_region_labels: [_dp_bb_run_76, _dp_bb_run_61, _dp_bb_run_77]
#                         except_region_labels: [_dp_bb_run_62, _dp_bb_run_63, _dp_bb_run_64]
#                 block _dp_bb_run_76:
#                     if __dp_is_(_dp_yield_from_sent_54, None):
#                         jump _dp_bb_run_61
#                     else:
#                         jump _dp_bb_run_77
#                 block _dp_bb_run_77:
#                     _dp_yield_from_y_53 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_54)
#                     jump _dp_bb_run_78
#                 block _dp_bb_run_51:
#                     jump _dp_bb_run_60
#                 block _dp_bb_run_end_50:
#                     return
#             else:
#                 return
#             _dp_with_exit_1 = None
#             jump _dp_bb_run_end_49
#             block _dp_bb_run_end_49:
#                 return
#         jump _dp_bb_run_end_7
#     jump _dp_bb_run_82
#     block _dp_bb_run_92:
#         _dp_yield_from_iter_83 = iter(__dp_await_iter(__dp_asynccontextmanager_aenter(cm)))
#         __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_83)
#         legacy_try_jump:
#             body_label: _dp_bb_run_93
#             except_label: _dp_bb_run_94
#             except_exc_name: _dp_try_exc_87
#             body_region_labels: [_dp_bb_run_93]
#             except_region_labels: [_dp_bb_run_94, _dp_bb_run_95, _dp_bb_run_96]
#     block _dp_bb_run_93:
#         _dp_yield_from_y_84 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#         jump _dp_bb_run_110
#     block _dp_bb_run_94:
#         if __dp_exception_matches(_dp_try_exc_87, StopIteration):
#             jump _dp_bb_run_95
#         else:
#             jump _dp_bb_run_96
#     block _dp_bb_run_95:
#         _dp_yield_from_result_86 = _dp_try_exc_87.value
#         jump _dp_bb_run_97
#     block _dp_bb_run_97:
#         __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#         jump _dp_bb_run_81
#     block _dp_bb_run_96:
#         _dp_yield_from_raise_89 = _dp_try_exc_87
#         jump _dp_bb_run_98
#     block _dp_bb_run_110:
#         __dp_store_cell(_dp_cell__dp_pc, 4)
#         return _dp_yield_from_y_84
#     block _dp_bb_run_99:
#         _dp_yield_from_sent_85 = _dp_send_value
#         _dp_yield_from_exc_88 = _dp_resume_exc
#         _dp_resume_exc = None
#         if __dp_is_not(_dp_yield_from_exc_88, None):
#             jump _dp_bb_run_100
#         else:
#             jump _dp_bb_run_107
#     block _dp_bb_run_100:
#         if __dp_exception_matches(_dp_yield_from_exc_88, GeneratorExit):
#             jump _dp_bb_run_101
#         else:
#             jump _dp_bb_run_104
#     block _dp_bb_run_101:
#         _dp_yield_from_close_90 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#         if __dp_is_not(_dp_yield_from_close_90, None):
#             jump _dp_bb_run_102
#         else:
#             jump _dp_bb_run_103
#     block _dp_bb_run_102:
#         _dp_yield_from_close_90()
#         jump _dp_bb_run_103
#     block _dp_bb_run_103:
#         _dp_yield_from_raise_89 = _dp_yield_from_exc_88
#         jump _dp_bb_run_98
#     block _dp_bb_run_98:
#         __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#         raise _dp_yield_from_raise_89
#     block _dp_bb_run_104:
#         _dp_yield_from_throw_91 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#         if __dp_is_(_dp_yield_from_throw_91, None):
#             jump _dp_bb_run_103
#         else:
#             jump _dp_bb_run_105
#     block _dp_bb_run_105:
#         legacy_try_jump:
#             body_label: _dp_bb_run_106
#             except_label: _dp_bb_run_94
#             except_exc_name: _dp_try_exc_87
#             body_region_labels: [_dp_bb_run_106]
#             except_region_labels: [_dp_bb_run_94, _dp_bb_run_95, _dp_bb_run_96]
#     block _dp_bb_run_106:
#         _dp_yield_from_y_84 = _dp_yield_from_throw_91(_dp_yield_from_exc_88)
#         jump _dp_bb_run_110
#     block _dp_bb_run_107:
#         legacy_try_jump:
#             body_label: _dp_bb_run_108
#             except_label: _dp_bb_run_94
#             except_exc_name: _dp_try_exc_87
#             body_region_labels: [_dp_bb_run_108, _dp_bb_run_93, _dp_bb_run_109]
#             except_region_labels: [_dp_bb_run_94, _dp_bb_run_95, _dp_bb_run_96]
#     block _dp_bb_run_108:
#         if __dp_is_(_dp_yield_from_sent_85, None):
#             jump _dp_bb_run_93
#         else:
#             jump _dp_bb_run_109
#     block _dp_bb_run_109:
#         _dp_yield_from_y_84 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_85)
#         jump _dp_bb_run_110
#     block _dp_bb_run_81:
#         x = _dp_yield_from_result_86
#         jump _dp_bb_run_80
#     block _dp_bb_run_82:
#         if False:
#             if __dp_is_not(_dp_resume_exc, None):
#                 pass
#             else:
#                 pass
#         else:
#             pass
#         _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
#         jump _dp_bb_run_92
#     block _dp_bb_run_end_7:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block run_dispatch_throw_done:
#         raise _dp_resume_exc
#     block run_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block run_dispatch_send_target_0:
#         jump _dp_bb_run_80
#     block run_dispatch_throw_target_0:
#         jump run_dispatch_throw_unstarted
#     block run_dispatch_send_target_1:
#         jump _dp_bb_run_11
#     block run_dispatch_throw_target_1:
#         jump _dp_bb_run_11
#     block run_dispatch_send_target_2:
#         jump _dp_bb_run_36
#     block run_dispatch_throw_target_2:
#         jump _dp_bb_run_36
#     block run_dispatch_send_target_3:
#         jump _dp_bb_run_67
#     block run_dispatch_throw_target_3:
#         jump _dp_bb_run_67
#     block run_dispatch_send_target_4:
#         jump _dp_bb_run_99
#     block run_dispatch_throw_target_4:
#         jump _dp_bb_run_99
#     block run_dispatch_invalid:
#         jump run_invalid
#     block run_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_dispatch_send_target_0, run_dispatch_send_target_1, run_dispatch_send_target_2, run_dispatch_send_target_3, run_dispatch_send_target_4] default run_dispatch_invalid
#     block run_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [run_dispatch_throw_target_0, run_dispatch_throw_target_1, run_dispatch_throw_target_2, run_dispatch_throw_target_3, run_dispatch_throw_target_4] default run_dispatch_invalid
#     block run_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump run_done
#         else:
#             jump run_dispatch_send_table
#     block run_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump run_dispatch_throw_done
#         else:
#             jump run_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump run_dispatch_send
#     else:
#         jump run_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def run(): ...

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_match_1 = value
#     if __dp_eq(_dp_match_1, 1):
#         one()
#     else:
#         other()

# generator_yield


def gen():
    yield 1


# ==

# module_init: _dp_module_init

# function gen() [kind=generator, bind=gen, target=module_global, qualname=gen]
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: start
#         resume_order: [start, _dp_bb_gen_3]
#         yield_sites:
#             _dp_bb_gen_1 -> _dp_bb_gen_3
#         done_block_label: gen_done
#         invalid_block_label: gen_invalid
#         uncaught_block_label: gen_uncaught
#         uncaught_set_done_label: gen_uncaught_set_done
#         uncaught_raise_label: gen_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_4
#         dispatch_only_labels: [gen_dispatch, gen_dispatch_invalid, gen_dispatch_send, gen_dispatch_send_table, gen_dispatch_send_target_0, gen_dispatch_send_target_1, gen_dispatch_throw, gen_dispatch_throw_done, gen_dispatch_throw_table, gen_dispatch_throw_target_0, gen_dispatch_throw_target_1, gen_dispatch_throw_unstarted]
#         throw_passthrough_labels: [gen_dispatch_throw_done, gen_dispatch_throw_unstarted, gen_uncaught_raise, gen_uncaught_set_done]
#     block gen_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block gen_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump gen_uncaught_set_done
#     else:
#         jump gen_uncaught_raise
#     block gen_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_4)
#         jump gen_uncaught_raise
#     block gen_uncaught_raise:
#         raise _dp_uncaught_exc_4
#     block start:
#         jump _dp_bb_gen_1
#     block _dp_bb_gen_2:
#         raise _dp_resume_exc
#     block _dp_bb_gen_3:
#         if __dp_is_not(_dp_resume_exc, None):
#             jump _dp_bb_gen_2
#         else:
#             jump _dp_bb_gen_end_0
#     block _dp_bb_gen_1:
#         __dp_store_cell(_dp_cell__dp_pc, 1)
#         return 1
#     block _dp_bb_gen_end_0:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block gen_dispatch_throw_done:
#         raise _dp_resume_exc
#     block gen_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block gen_dispatch_send_target_0:
#         jump start
#     block gen_dispatch_throw_target_0:
#         jump gen_dispatch_throw_unstarted
#     block gen_dispatch_send_target_1:
#         jump _dp_bb_gen_3
#     block gen_dispatch_throw_target_1:
#         jump _dp_bb_gen_3
#     block gen_dispatch_invalid:
#         jump gen_invalid
#     block gen_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_dispatch_send_target_0, gen_dispatch_send_target_1] default gen_dispatch_invalid
#     block gen_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_dispatch_throw_target_0, gen_dispatch_throw_target_1] default gen_dispatch_invalid
#     block gen_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump gen_done
#         else:
#             jump gen_dispatch_send_table
#     block gen_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump gen_dispatch_throw_done
#         else:
#             jump gen_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump gen_dispatch_send
#     else:
#         jump gen_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def gen(): ...

# yield_from


def gen():
    yield from it


# ==

# module_init: _dp_module_init

# function gen() [kind=generator, bind=gen, target=module_global, qualname=gen]
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: start
#         resume_order: [start, _dp_bb_gen_17]
#         yield_sites:
#             _dp_bb_gen_28 -> _dp_bb_gen_17
#         done_block_label: gen_done
#         invalid_block_label: gen_invalid
#         uncaught_block_label: gen_uncaught
#         uncaught_set_done_label: gen_uncaught_set_done
#         uncaught_raise_label: gen_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_29
#         dispatch_only_labels: [gen_dispatch, gen_dispatch_invalid, gen_dispatch_send, gen_dispatch_send_table, gen_dispatch_send_target_0, gen_dispatch_send_target_1, gen_dispatch_throw, gen_dispatch_throw_done, gen_dispatch_throw_table, gen_dispatch_throw_target_0, gen_dispatch_throw_target_1, gen_dispatch_throw_unstarted]
#         throw_passthrough_labels: [gen_dispatch_throw_done, gen_dispatch_throw_unstarted, gen_uncaught_raise, gen_uncaught_set_done]
#     block gen_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block gen_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump gen_uncaught_set_done
#     else:
#         jump gen_uncaught_raise
#     block gen_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_29)
#         jump gen_uncaught_raise
#     block gen_uncaught_raise:
#         raise _dp_uncaught_exc_29
#     block start:
#         jump _dp_bb_gen_1
#     block _dp_bb_gen_10:
#         _dp_yield_from_iter_2 = iter(it)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, _dp_yield_from_iter_2)
#         legacy_try_jump:
#             body_label: _dp_bb_gen_11
#             except_label: _dp_bb_gen_12
#             except_exc_name: _dp_try_exc_5
#             body_region_labels: [_dp_bb_gen_11]
#             except_region_labels: [_dp_bb_gen_12, _dp_bb_gen_13, _dp_bb_gen_14]
#     block _dp_bb_gen_11:
#         _dp_yield_from_y_3 = next(__dp_load_cell(_dp_cell__dp_yieldfrom))
#         jump _dp_bb_gen_28
#     block _dp_bb_gen_12:
#         if __dp_exception_matches(_dp_try_exc_5, StopIteration):
#             jump _dp_bb_gen_13
#         else:
#             jump _dp_bb_gen_14
#     block _dp_bb_gen_13:
#         jump _dp_bb_gen_15
#     block _dp_bb_gen_15:
#         __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#         jump _dp_bb_gen_end_0
#     block _dp_bb_gen_14:
#         _dp_yield_from_raise_7 = _dp_try_exc_5
#         jump _dp_bb_gen_16
#     block _dp_bb_gen_28:
#         __dp_store_cell(_dp_cell__dp_pc, 1)
#         return _dp_yield_from_y_3
#     block _dp_bb_gen_17:
#         _dp_yield_from_sent_4 = _dp_send_value
#         _dp_yield_from_exc_6 = _dp_resume_exc
#         _dp_resume_exc = None
#         if __dp_is_not(_dp_yield_from_exc_6, None):
#             jump _dp_bb_gen_18
#         else:
#             jump _dp_bb_gen_25
#     block _dp_bb_gen_18:
#         if __dp_exception_matches(_dp_yield_from_exc_6, GeneratorExit):
#             jump _dp_bb_gen_19
#         else:
#             jump _dp_bb_gen_22
#     block _dp_bb_gen_19:
#         _dp_yield_from_close_8 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "close", None)
#         if __dp_is_not(_dp_yield_from_close_8, None):
#             jump _dp_bb_gen_20
#         else:
#             jump _dp_bb_gen_21
#     block _dp_bb_gen_20:
#         _dp_yield_from_close_8()
#         jump _dp_bb_gen_21
#     block _dp_bb_gen_21:
#         _dp_yield_from_raise_7 = _dp_yield_from_exc_6
#         jump _dp_bb_gen_16
#     block _dp_bb_gen_16:
#         __dp_store_cell(_dp_cell__dp_yieldfrom, None)
#         raise _dp_yield_from_raise_7
#     block _dp_bb_gen_22:
#         _dp_yield_from_throw_9 = getattr(__dp_load_cell(_dp_cell__dp_yieldfrom), "throw", None)
#         if __dp_is_(_dp_yield_from_throw_9, None):
#             jump _dp_bb_gen_21
#         else:
#             jump _dp_bb_gen_23
#     block _dp_bb_gen_23:
#         legacy_try_jump:
#             body_label: _dp_bb_gen_24
#             except_label: _dp_bb_gen_12
#             except_exc_name: _dp_try_exc_5
#             body_region_labels: [_dp_bb_gen_24]
#             except_region_labels: [_dp_bb_gen_12, _dp_bb_gen_13, _dp_bb_gen_14]
#     block _dp_bb_gen_24:
#         _dp_yield_from_y_3 = _dp_yield_from_throw_9(_dp_yield_from_exc_6)
#         jump _dp_bb_gen_28
#     block _dp_bb_gen_25:
#         legacy_try_jump:
#             body_label: _dp_bb_gen_26
#             except_label: _dp_bb_gen_12
#             except_exc_name: _dp_try_exc_5
#             body_region_labels: [_dp_bb_gen_26, _dp_bb_gen_11, _dp_bb_gen_27]
#             except_region_labels: [_dp_bb_gen_12, _dp_bb_gen_13, _dp_bb_gen_14]
#     block _dp_bb_gen_26:
#         if __dp_is_(_dp_yield_from_sent_4, None):
#             jump _dp_bb_gen_11
#         else:
#             jump _dp_bb_gen_27
#     block _dp_bb_gen_27:
#         _dp_yield_from_y_3 = __dp_load_cell(_dp_cell__dp_yieldfrom).send(_dp_yield_from_sent_4)
#         jump _dp_bb_gen_28
#     block _dp_bb_gen_1:
#         jump _dp_bb_gen_10
#     block _dp_bb_gen_end_0:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block gen_dispatch_throw_done:
#         raise _dp_resume_exc
#     block gen_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block gen_dispatch_send_target_0:
#         jump start
#     block gen_dispatch_throw_target_0:
#         jump gen_dispatch_throw_unstarted
#     block gen_dispatch_send_target_1:
#         jump _dp_bb_gen_17
#     block gen_dispatch_throw_target_1:
#         jump _dp_bb_gen_17
#     block gen_dispatch_invalid:
#         jump gen_invalid
#     block gen_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_dispatch_send_target_0, gen_dispatch_send_target_1] default gen_dispatch_invalid
#     block gen_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [gen_dispatch_throw_target_0, gen_dispatch_throw_target_1] default gen_dispatch_invalid
#     block gen_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump gen_done
#         else:
#             jump gen_dispatch_send_table
#     block gen_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump gen_dispatch_throw_done
#         else:
#             jump gen_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump gen_dispatch_send
#     else:
#         jump gen_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def gen(): ...

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_tmp_4 = Suppress()
#     _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_4)
#     __dp_contextmanager_enter(_dp_tmp_4)
#     _dp_with_ok_2 = True
#     try:
#         raise RuntimeError("boom")
#     except BaseException:
#         _dp_with_ok_2 = False
#         __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
#     else:
#         pass
#     finally:
#         if _dp_with_ok_2:
#             __dp_contextmanager_exit(_dp_with_exit_1, None)
#         _dp_with_exit_1 = None
#         _dp_tmp_4 = None

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==

# module_init: _dp_module_init

# function inner() [kind=function, bind=inner, target=local, qualname=outer.<locals>.inner]
#     return __dp_load_cell(_dp_cell_x)

# function outer() [kind=function, bind=outer, target=module_global, qualname=outer]
#     _dp_cell_x = __dp_make_cell()
#     __dp_store_cell(_dp_cell_x, 5)
#     def inner(): ...
#     return inner()

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def outer(): ...

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
#     total = __dp_add(a, b)
#     if __dp_gt(total, 5):
#         return a
#     else:
#         return b

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def choose(a, b): ...

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
#     __dp_store_cell(_dp_cell_x, 2)
#     return __dp_load_cell(_dp_cell_x)

# function outer() [kind=function, bind=outer, target=module_global, qualname=outer]
#     _dp_cell_x = __dp_make_cell()
#     __dp_store_cell(_dp_cell_x, 5)
#     def inner(): ...
#     return inner()

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def outer(): ...

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     try:
#         print(1)
#     except:
#         if __dp_exception_matches(__dp_current_exception(), Exception):
#             print(2)
#         else:
#             raise
#     else:
#         pass
#     finally:
#         pass

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
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: start
#         resume_order: [start, _dp_bb_complicated_19]
#         yield_sites:
#             _dp_bb_complicated_17 -> _dp_bb_complicated_19
#         done_block_label: complicated_done
#         invalid_block_label: complicated_invalid
#         uncaught_block_label: complicated_uncaught
#         uncaught_set_done_label: complicated_uncaught_set_done
#         uncaught_raise_label: complicated_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_25
#         dispatch_only_labels: [complicated_dispatch, complicated_dispatch_invalid, complicated_dispatch_send, complicated_dispatch_send_table, complicated_dispatch_send_target_0, complicated_dispatch_send_target_1, complicated_dispatch_throw, complicated_dispatch_throw_done, complicated_dispatch_throw_table, complicated_dispatch_throw_target_0, complicated_dispatch_throw_target_1, complicated_dispatch_throw_unstarted]
#         throw_passthrough_labels: [complicated_dispatch_throw_done, complicated_dispatch_throw_unstarted, complicated_uncaught_raise, complicated_uncaught_set_done]
#     block complicated_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block complicated_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump complicated_uncaught_set_done
#     else:
#         jump complicated_uncaught_raise
#     block complicated_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_25)
#         jump complicated_uncaught_raise
#     block complicated_uncaught_raise:
#         raise _dp_uncaught_exc_25
#     block start:
#         jump for_setup_0
#     block for_setup_0:
#         _dp_iter_1 = __dp_iter(a)
#         jump for_fetch_1
#     block for_fetch_1:
#         _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#         if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#             jump for_else_4
#             return
#         else:
#             i = _dp_tmp_2
#             _dp_tmp_2 = None
#             jump for_body_2
#             return
#     block for_body_2:
#         try:
#             jump _dp_bb_complicated_17
#             block _dp_bb_complicated_18:
#                 raise _dp_resume_exc
#             if __dp_is_not(_dp_resume_exc, None):
#                 jump _dp_bb_complicated_18
#             else:
#                 jump _dp_bb_complicated_end_16
#             block _dp_bb_complicated_17:
#                 j = __dp_add(i, 1)
#                 return j
#             block _dp_bb_complicated_end_16:
#                 return
#         except:
#             if __dp_exception_matches(__dp_current_exception(), Exception):
#                 print("oops")
#                 jump _dp_bb_complicated_end_21
#                 block _dp_bb_complicated_end_21:
#                     return
#             else:
#                 raise
#                 return
#             return
#         else:
#             jump _dp_bb_complicated_end_23
#             block _dp_bb_complicated_end_23:
#                 return
#         finally:
#             jump _dp_bb_complicated_end_24
#             block _dp_bb_complicated_end_24:
#                 return
#         jump for_fetch_1
#     block for_else_4:
#         print("finsihed")
#         jump for_after_3
#     block for_after_3:
#         jump _dp_bb_complicated_end_13
#     block _dp_bb_complicated_end_13:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block complicated_dispatch_throw_done:
#         raise _dp_resume_exc
#     block complicated_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block complicated_dispatch_send_target_0:
#         jump start
#     block complicated_dispatch_throw_target_0:
#         jump complicated_dispatch_throw_unstarted
#     block complicated_dispatch_send_target_1:
#         jump _dp_bb_complicated_19
#     block complicated_dispatch_throw_target_1:
#         jump _dp_bb_complicated_19
#     block complicated_dispatch_invalid:
#         jump complicated_invalid
#     block complicated_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [complicated_dispatch_send_target_0, complicated_dispatch_send_target_1] default complicated_dispatch_invalid
#     block complicated_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [complicated_dispatch_throw_target_0, complicated_dispatch_throw_target_1] default complicated_dispatch_invalid
#     block complicated_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump complicated_done
#         else:
#             jump complicated_dispatch_send_table
#     block complicated_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump complicated_dispatch_throw_done
#         else:
#             jump complicated_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump complicated_dispatch_send
#     else:
#         jump complicated_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def complicated(a): ...

# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")


# ==

# module_init: _dp_module_init

# function complicated(a) [kind=generator, bind=complicated, target=module_global, qualname=complicated]
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: start
#         resume_order: [start, _dp_bb_complicated_18]
#         yield_sites:
#             _dp_bb_complicated_16 -> _dp_bb_complicated_18
#         done_block_label: complicated_done
#         invalid_block_label: complicated_invalid
#         uncaught_block_label: complicated_uncaught
#         uncaught_set_done_label: complicated_uncaught_set_done
#         uncaught_raise_label: complicated_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_24
#         dispatch_only_labels: [complicated_dispatch, complicated_dispatch_invalid, complicated_dispatch_send, complicated_dispatch_send_table, complicated_dispatch_send_target_0, complicated_dispatch_send_target_1, complicated_dispatch_throw, complicated_dispatch_throw_done, complicated_dispatch_throw_table, complicated_dispatch_throw_target_0, complicated_dispatch_throw_target_1, complicated_dispatch_throw_unstarted]
#         throw_passthrough_labels: [complicated_dispatch_throw_done, complicated_dispatch_throw_unstarted, complicated_uncaught_raise, complicated_uncaught_set_done]
#     block complicated_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block complicated_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump complicated_uncaught_set_done
#     else:
#         jump complicated_uncaught_raise
#     block complicated_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_24)
#         jump complicated_uncaught_raise
#     block complicated_uncaught_raise:
#         raise _dp_uncaught_exc_24
#     block start:
#         jump for_setup_0
#     block for_setup_0:
#         _dp_iter_1 = __dp_iter(a)
#         jump for_fetch_1
#     block for_fetch_1:
#         _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#         if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#             jump for_after_3
#             return
#         else:
#             i = _dp_tmp_2
#             _dp_tmp_2 = None
#             jump for_body_2
#             return
#     block for_body_2:
#         try:
#             jump _dp_bb_complicated_16
#             block _dp_bb_complicated_17:
#                 raise _dp_resume_exc
#             if __dp_is_not(_dp_resume_exc, None):
#                 jump _dp_bb_complicated_17
#             else:
#                 jump _dp_bb_complicated_end_15
#             block _dp_bb_complicated_16:
#                 j = __dp_add(i, 1)
#                 return j
#             block _dp_bb_complicated_end_15:
#                 return
#         except:
#             if __dp_exception_matches(__dp_current_exception(), Exception):
#                 print("oops")
#                 jump _dp_bb_complicated_end_20
#                 block _dp_bb_complicated_end_20:
#                     return
#             else:
#                 raise
#                 return
#             return
#         else:
#             jump _dp_bb_complicated_end_22
#             block _dp_bb_complicated_end_22:
#                 return
#         finally:
#             jump _dp_bb_complicated_end_23
#             block _dp_bb_complicated_end_23:
#                 return
#         jump for_fetch_1
#     block for_after_3:
#         jump _dp_bb_complicated_end_12
#     block _dp_bb_complicated_end_12:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block complicated_dispatch_throw_done:
#         raise _dp_resume_exc
#     block complicated_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block complicated_dispatch_send_target_0:
#         jump start
#     block complicated_dispatch_throw_target_0:
#         jump complicated_dispatch_throw_unstarted
#     block complicated_dispatch_send_target_1:
#         jump _dp_bb_complicated_18
#     block complicated_dispatch_throw_target_1:
#         jump _dp_bb_complicated_18
#     block complicated_dispatch_invalid:
#         jump complicated_invalid
#     block complicated_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [complicated_dispatch_send_target_0, complicated_dispatch_send_target_1] default complicated_dispatch_invalid
#     block complicated_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [complicated_dispatch_throw_target_0, complicated_dispatch_throw_target_1] default complicated_dispatch_invalid
#     block complicated_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump complicated_done
#         else:
#             jump complicated_dispatch_send_table
#     block complicated_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump complicated_dispatch_throw_done
#         else:
#             jump complicated_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump complicated_dispatch_send
#     else:
#         jump complicated_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def complicated(a): ...
