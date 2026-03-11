# subscript

x = a[b]

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_getitem(a, b))

# subscript_slice

x = a[1:2:3]

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_getitem(a, __dp_slice(1, 2, 3)))

# binary_add

x = a + b

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_add(a, b))

# binary_bitwise_or

x = a | b

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_or_(a, b))

# unary_neg

x = -a

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_neg(a))

# boolop_chain

x = a and b or c

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_target_1 = a
#     if _dp_target_1:
#         _dp_target_1 = b
#     if __dp_not_(_dp_target_1):
#         _dp_target_1 = c
#     __dp_store_global(globals(), "x", _dp_target_1)

# compare_lt

x = a < b

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_lt(a, b))

# compare_chain

x = a < b < c

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     _dp_compare_2 = a
#     _dp_compare_3 = b
#     _dp_target_1 = __dp_lt(_dp_compare_2, _dp_compare_3)
#     if _dp_target_1:
#         _dp_target_1 = __dp_lt(_dp_compare_3, c)
#     __dp_store_global(globals(), "x", _dp_target_1)

# compare_not_in

x = a not in b

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_not_(__dp_contains(b, a)))

# if_expr

x = a if cond else b

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     if cond:
#         _dp_tmp_1 = a
#     else:
#         _dp_tmp_1 = b
#     __dp_store_global(globals(), "x", _dp_tmp_1)

# named_expr

x = (y := f())

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "y", f())
#     __dp_store_global(globals(), "x", __dp_load_global(globals(), "y"))

# lambda_simple

x = lambda y: y + 1

# ==

# module_init: _dp_module_init

# function _dp_lambda_1(y) [kind=function, bind=_dp_lambda_1, target=local, qualname=<lambda>]
#     return __dp_add(y, 1)

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_lambda_1(y): ...
#     __dp_store_global(globals(), "x", _dp_lambda_1)

# generator_expr

x = (i for i in it)

# ==

# module_init: _dp_module_init

# function _dp_genexpr_1(_dp_iter_2) [kind=generator, bind=_dp_genexpr_1, target=local, qualname=<genexpr>]
#     generator_state:
#         closure_state: true
#         dispatch_entry_label: start
#         resume_order: [start, _dp_bb__dp_genexpr_1_14]
#         yield_sites:
#             _dp_bb__dp_genexpr_1_12 -> _dp_bb__dp_genexpr_1_14
#         done_block_label: _dp_genexpr_1_done
#         invalid_block_label: _dp_genexpr_1_invalid
#         uncaught_block_label: _dp_genexpr_1_uncaught
#         uncaught_set_done_label: _dp_genexpr_1_uncaught_set_done
#         uncaught_raise_label: _dp_genexpr_1_uncaught_raise
#         uncaught_exc_name: _dp_uncaught_exc_15
#         dispatch_only_labels: [_dp_genexpr_1_dispatch, _dp_genexpr_1_dispatch_invalid, _dp_genexpr_1_dispatch_send, _dp_genexpr_1_dispatch_send_table, _dp_genexpr_1_dispatch_send_target_0, _dp_genexpr_1_dispatch_send_target_1, _dp_genexpr_1_dispatch_throw, _dp_genexpr_1_dispatch_throw_done, _dp_genexpr_1_dispatch_throw_table, _dp_genexpr_1_dispatch_throw_target_0, _dp_genexpr_1_dispatch_throw_target_1, _dp_genexpr_1_dispatch_throw_unstarted]
#         throw_passthrough_labels: [_dp_genexpr_1_dispatch_throw_done, _dp_genexpr_1_dispatch_throw_unstarted, _dp_genexpr_1_uncaught_raise, _dp_genexpr_1_uncaught_set_done]
#     block _dp_genexpr_1_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block _dp_genexpr_1_invalid:
#         raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     if __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#         jump _dp_genexpr_1_uncaught_set_done
#     else:
#         jump _dp_genexpr_1_uncaught_raise
#     block _dp_genexpr_1_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_15)
#         jump _dp_genexpr_1_uncaught_raise
#     block _dp_genexpr_1_uncaught_raise:
#         raise _dp_uncaught_exc_15
#     block start:
#         _dp_iter_3 = _dp_iter_2
#         jump while_test_0
#     block while_test_0:
#         if True:
#             jump while_body_1
#             return
#         else:
#             jump while_after_2
#             return
#     block while_body_1:
#         _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
#         if __dp_is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
#             jump while_after_2
#             return
#         else:
#             jump _dp_bb__dp_genexpr_1_12
#             block _dp_bb__dp_genexpr_1_13:
#                 raise _dp_resume_exc
#             block _dp_bb__dp_genexpr_1_14:
#                 if __dp_is_not(_dp_resume_exc, None):
#                     jump _dp_bb__dp_genexpr_1_13
#                 else:
#                     jump _dp_bb__dp_genexpr_1_end_11
#             block _dp_bb__dp_genexpr_1_12:
#                 i = _dp_tmp_4
#                 return i
#             block _dp_bb__dp_genexpr_1_end_11:
#                 return
#     block while_after_2:
#         jump _dp_bb__dp_genexpr_1_end_7
#     block _dp_bb__dp_genexpr_1_end_7:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         raise StopIteration()
#     block _dp_genexpr_1_dispatch_throw_done:
#         raise _dp_resume_exc
#     block _dp_genexpr_1_dispatch_throw_unstarted:
#         raise _dp_resume_exc
#     block _dp_genexpr_1_dispatch_send_target_0:
#         jump start
#     block _dp_genexpr_1_dispatch_throw_target_0:
#         jump _dp_genexpr_1_dispatch_throw_unstarted
#     block _dp_genexpr_1_dispatch_send_target_1:
#         jump _dp_bb__dp_genexpr_1_14
#     block _dp_genexpr_1_dispatch_throw_target_1:
#         jump _dp_bb__dp_genexpr_1_14
#     block _dp_genexpr_1_dispatch_invalid:
#         jump _dp_genexpr_1_invalid
#     block _dp_genexpr_1_dispatch_send_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_genexpr_1_dispatch_send_target_0, _dp_genexpr_1_dispatch_send_target_1] default _dp_genexpr_1_dispatch_invalid
#     block _dp_genexpr_1_dispatch_throw_table:
#         branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_genexpr_1_dispatch_throw_target_0, _dp_genexpr_1_dispatch_throw_target_1] default _dp_genexpr_1_dispatch_invalid
#     block _dp_genexpr_1_dispatch_send:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump _dp_genexpr_1_done
#         else:
#             jump _dp_genexpr_1_dispatch_send_table
#     block _dp_genexpr_1_dispatch_throw:
#         if __dp_eq(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             jump _dp_genexpr_1_dispatch_throw_done
#         else:
#             jump _dp_genexpr_1_dispatch_throw_table
#     if __dp_is_(_dp_resume_exc, None):
#         jump _dp_genexpr_1_dispatch_send
#     else:
#         jump _dp_genexpr_1_dispatch_throw

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_genexpr_1(_dp_iter_2): ...
#     __dp_store_global(globals(), "x", _dp_genexpr_1(__dp_iter(it)))

# list_literal

x = [a, b]

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_list(__dp_tuple(a, b)))

# list_literal_splat

x = [a, *b]

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_list(__dp_add(__dp_tuple(a), __dp_tuple_from_iter(b))))

# tuple_splat

x = (a, *b)

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_add(__dp_tuple(a), __dp_tuple_from_iter(b)))

# set_literal

x = {a, b}

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_set(__dp_tuple(a, b)))

# dict_literal

x = {"a": 1, "b": 2}

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_dict(__dp_tuple(("a", 1), ("b", 2))))

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_or_(__dp_or_(__dp_dict(__dp_tuple(("a", 1))), __dp_dict(m)), __dp_dict(__dp_tuple(("b", 2)))))

# list_comp

x = [i for i in it]

# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2) [kind=function, bind=_dp_listcomp_3, target=local, qualname=_dp_listcomp_3]
#     _dp_tmp_1 = __dp_list(__dp_tuple())
#     _dp_iter_4 = __dp_iter(_dp_iter_2)
#     block for_fetch_1:
#         _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
#         if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             i = _dp_tmp_5
#             _dp_tmp_5 = None
#             jump for_body_2
#     block for_body_2:
#         _dp_tmp_1.append(i)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_1

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_listcomp_3(_dp_iter_2): ...
#     __dp_store_global(globals(), "x", _dp_listcomp_3(it))

# set_comp

x = {i for i in it}

# ==

# module_init: _dp_module_init

# function _dp_setcomp_3(_dp_iter_2) [kind=function, bind=_dp_setcomp_3, target=local, qualname=_dp_setcomp_3]
#     _dp_tmp_1 = set()
#     _dp_iter_4 = __dp_iter(_dp_iter_2)
#     block for_fetch_1:
#         _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
#         if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             i = _dp_tmp_5
#             _dp_tmp_5 = None
#             jump for_body_2
#     block for_body_2:
#         _dp_tmp_1.add(i)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_1

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_setcomp_3(_dp_iter_2): ...
#     __dp_store_global(globals(), "x", _dp_setcomp_3(it))

# dict_comp

x = {k: v for k, v in it}

# ==

# module_init: _dp_module_init

# function _dp_dictcomp_3(_dp_iter_2) [kind=function, bind=_dp_dictcomp_3, target=local, qualname=_dp_dictcomp_3]
#     _dp_tmp_1 = __dp_dict()
#     _dp_iter_4 = __dp_iter(_dp_iter_2)
#     block for_fetch_1:
#         _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
#         if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
#             jump for_after_3
#         else:
#             _dp_tmp_6 = __dp_unpack(_dp_tmp_5, __dp_tuple(True, True))
#             k = __dp_getitem(_dp_tmp_6, 0)
#             v = __dp_getitem(_dp_tmp_6, 1)
#             del _dp_tmp_6
#             _dp_tmp_5 = None
#             jump for_body_2
#     block for_body_2:
#         __dp_setitem(_dp_tmp_1, k, v)
#         jump for_fetch_1
#     block for_after_3:
#         return _dp_tmp_1

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     def _dp_dictcomp_3(_dp_iter_2): ...
#     __dp_store_global(globals(), "x", _dp_dictcomp_3(it))

# attribute_non_chain

x = f().y

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", f().y)

# fstring_simple

x = f"{a}"

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_format(a))

# tstring_simple

x = t"{a}"

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_templatelib_Template(*__dp_tuple(__dp_templatelib_Interpolation(a, "a", None, ""))))

# complex_literal

x = 1j

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", complex(0.0, 1.0))

# float_literal_long

x = 1.234567890123456789

# ==

# module_init: _dp_module_init

# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", __dp_float_from_literal("1.234567890123456789"))
