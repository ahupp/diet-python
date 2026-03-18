# subscript

x = a[b]

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", a[b])
#         return

# subscript_slice

x = a[1:2:3]

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", a[1:2:3])
#         return

# binary_add

x = a + b

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", a + b)
#         return

# binary_bitwise_or

x = a | b

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", a | b)
#         return

# unary_neg

x = -a

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", -a)
#         return

# boolop_chain

x = a and b or c

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_target_2 = a
#         if _dp_target_2:
#             _dp_target_2 = b
#         _dp_target_1 = _dp_target_2
#         if not _dp_target_1:
#             _dp_target_1 = c
#         __dp_store_global(globals(), "x", _dp_target_1)
#         return

# compare_lt

x = a < b

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", __dp_lt(a, b))
#         return

# compare_chain

x = a < b < c

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         _dp_compare_1 = a
#         _dp_compare_3 = b
#         _dp_target_2 = __dp_lt(_dp_compare_1, _dp_compare_3)
#         if _dp_target_2:
#             _dp_target_2 = __dp_lt(_dp_compare_3, c)
#         __dp_store_global(globals(), "x", _dp_target_2)
#         return

# compare_not_in

x = a not in b

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", __dp_not_(__dp_contains(b, a)))
#         return

# if_expr

x = a if cond else b

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         if cond:
#             _dp_tmp_1 = a
#         else:
#             _dp_tmp_1 = b
#         __dp_store_global(globals(), "x", _dp_tmp_1)
#         return

# named_expr

x = (y := f())

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", __dp_store_global(globals(), "y", f()))
#         return

# lambda_simple

x = lambda y: y + 1

# ==

# module_init: _dp_module_init

# function <lambda>(y):
#     display_name: <lambda>
#     block start:
#         return y + 1

# function _dp_module_init():
#     block start:
#         _dp_lambda_1 = __dp_make_function("start", 0, "<lambda>", "<lambda>", __dp_tuple("y"), __dp_tuple(__dp_tuple("y", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "x", _dp_lambda_1)
#         return

# generator_expr

x = (i for i in it)

# ==

# module_init: _dp_module_init

# generator <genexpr>(_dp_iter_2):
#     display_name: _dp_resume
#     entry_liveins: [_dp_self, _dp_send_value, _dp_resume_exc, _dp_cell__dp_iter_2, _dp_cell__dp_iter_3, _dp_cell__dp_tmp_4, _dp_cell_i, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom]
#     local_cell_slots: [_dp_cell__dp_iter_2, _dp_cell__dp_iter_3, _dp_cell__dp_pc, _dp_cell__dp_tmp_4, _dp_cell__dp_yieldfrom, _dp_cell_i]
#     cellvars: [_dp_iter_2->_dp_cell__dp_iter_2@param, _dp_iter_3->_dp_cell__dp_iter_3@deferred, _dp_tmp_4->_dp_cell__dp_tmp_4@deferred, i->_dp_cell_i@deferred]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted, _dp_yieldfrom->_dp_cell__dp_yieldfrom@none]
#     block start:
#         if_term __dp_is_(_dp_resume_exc, None):
#             then:
#                 block _dp_bb__dp_genexpr_1_dispatch_send_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb__dp_genexpr_1_done, _dp_bb__dp_genexpr_1_start, _dp_bb__dp_genexpr_1_2] default _dp_bb__dp_genexpr_1_invalid
#                     block _dp_bb__dp_genexpr_1_done:
#                         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                         jump _dp_bb__dp_genexpr_1_done_return_done
#                         block _dp_bb__dp_genexpr_1_done_return_done:
#                             raise StopIteration()
#                     block _dp_bb__dp_genexpr_1_start:
#                         _dp_iter_2 = __dp_load_deleted_name("_dp_iter_2", __dp_load_cell(_dp_cell__dp_iter_2))
#                         __dp_store_cell(_dp_cell__dp_iter_2, _dp_iter_2)
#                         _dp_iter_3 = _dp_iter_2
#                         __dp_store_cell(_dp_cell__dp_iter_3, _dp_iter_3)
#                         jump _dp_bb__dp_genexpr_1_5
#             else:
#                 block _dp_bb__dp_genexpr_1_dispatch_throw_table:
#                     branch_table __dp_load_cell(_dp_cell__dp_pc) -> [_dp_bb__dp_genexpr_1_dispatch_throw_done, _dp_bb__dp_genexpr_1_dispatch_throw_unstarted, _dp_bb__dp_genexpr_1_2] default _dp_bb__dp_genexpr_1_invalid
#                     block _dp_bb__dp_genexpr_1_dispatch_throw_done:
#                         raise _dp_resume_exc
#                     block _dp_bb__dp_genexpr_1_dispatch_throw_unstarted:
#                         raise _dp_resume_exc
#         block _dp_bb__dp_genexpr_1_2:
#             if_term __dp_is_not(_dp_resume_exc, None):
#                 then:
#                     block _dp_bb__dp_genexpr_1_1:
#                         raise _dp_resume_exc
#                 else:
#                     jump _dp_bb__dp_genexpr_1_5
#         block _dp_bb__dp_genexpr_1_5:
#             jump _dp_bb__dp_genexpr_1_4
#             block _dp_bb__dp_genexpr_1_4:
#                 _dp_iter_3 = __dp_load_deleted_name("_dp_iter_3", __dp_load_cell(_dp_cell__dp_iter_3))
#                 __dp_store_cell(_dp_cell__dp_iter_3, _dp_iter_3)
#                 _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
#                 __dp_store_cell(_dp_cell__dp_tmp_4, _dp_tmp_4)
#                 if_term __dp_is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
#                     then:
#                         block _dp_bb__dp_genexpr_1_0:
#                             __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#                             jump _dp_bb__dp_genexpr_1_0_return_done
#                             block _dp_bb__dp_genexpr_1_0_return_done:
#                                 raise StopIteration()
#                     else:
#                         block _dp_bb__dp_genexpr_1_3:
#                             _dp_tmp_4 = __dp_load_deleted_name("_dp_tmp_4", __dp_load_cell(_dp_cell__dp_tmp_4))
#                             __dp_store_cell(_dp_cell__dp_tmp_4, _dp_tmp_4)
#                             i = _dp_tmp_4
#                             __dp_store_cell(_dp_cell_i, i)
#                             __dp_store_cell(_dp_cell__dp_pc, 2)
#                             return i
#         block _dp_bb__dp_genexpr_1_invalid:
#             raise RuntimeError("invalid generator pc: {}".format(__dp_load_cell(_dp_cell__dp_pc)))
#     block _dp_bb__dp_genexpr_1_uncaught:
#         if_term __dp_ne(__dp_load_cell(_dp_cell__dp_pc), __dp_GEN_PC_DONE):
#             then:
#                 jump _dp_bb__dp_genexpr_1_uncaught_set_done
#             else:
#                 jump _dp_bb__dp_genexpr_1_uncaught_raise
#     block _dp_bb__dp_genexpr_1_uncaught_raise:
#         raise _dp_uncaught_exc_8
#     block _dp_bb__dp_genexpr_1_uncaught_set_done:
#         __dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)
#         __dp_store_cell(_dp_cell__dp_iter_2, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_iter_3, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_tmp_4, __dp_DELETED)
#         __dp_store_cell(_dp_cell_i, __dp_DELETED)
#         __dp_store_cell(_dp_cell__dp_yieldfrom, __dp_DELETED)
#         __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_8)
#         jump _dp_bb__dp_genexpr_1_uncaught_raise

# function <genexpr>(_dp_iter_2):
#     display_name: <genexpr>
#     local_cell_slots: [_dp_cell__dp_iter_2, _dp_cell__dp_iter_3, _dp_cell__dp_pc, _dp_cell__dp_tmp_4, _dp_cell__dp_yieldfrom, _dp_cell_i]
#     block start:
#         _dp_cell__dp_iter_2 = __dp_make_cell(_dp_iter_2)
#         _dp_cell__dp_iter_3 = __dp_make_cell(None)
#         _dp_cell__dp_tmp_4 = __dp_make_cell(None)
#         _dp_cell_i = __dp_make_cell(None)
#         _dp_cell__dp_pc = __dp_make_cell(1)
#         _dp_cell__dp_yieldfrom = __dp_make_cell(None)
#         return __dp_make_closure_generator(__dp_def_hidden_resume_fn("start", 2, "_dp_resume", "<genexpr>", __dp_tuple("_dp_self", "_dp_send_value", "_dp_resume_exc", "_dp_cell__dp_iter_2", "_dp_cell__dp_iter_3", "_dp_cell__dp_tmp_4", "_dp_cell_i", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple("_dp_cell__dp_iter_2", "_dp_cell__dp_iter_3", "_dp_cell__dp_tmp_4", "_dp_cell_i", "_dp_cell__dp_pc", "_dp_cell__dp_yieldfrom"), __dp_tuple(_dp_cell__dp_iter_2, _dp_cell__dp_iter_3, _dp_cell__dp_tmp_4, _dp_cell_i, _dp_cell__dp_pc, _dp_cell__dp_yieldfrom), __dp_globals(), __name__, async_gen=False), "<genexpr>", "<genexpr>")

# function _dp_module_init():
#     entry_liveins: [_dp_cell__dp_pc]
#     runtime_cells: [_dp_pc->_dp_cell__dp_pc@pc_unstarted]
#     block start:
#         _dp_genexpr_1 = __dp_make_function("start", 0, "<genexpr>", "<genexpr>", __dp_tuple("_dp_iter_2", __dp_tuple("_dp_cell__dp_pc", _dp_cell__dp_pc), "_dp_iter_3"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "x", _dp_genexpr_1(__dp_iter(it)))
#         return

# list_literal

x = [a, b]

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", [a, b])
#         return

# list_literal_splat

x = [a, *b]

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", [a, *b])
#         return

# tuple_splat

x = (a, *b)

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", (a, *b))
#         return

# set_literal

x = {a, b}

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", {a, b})
#         return

# dict_literal

x = {"a": 1, "b": 2}

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", {"a": 1, "b": 2})
#         return

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", {"a": 1, **m, "b": 2})
#         return

# list_comp

x = [i for i in it]

# ==

# module_init: _dp_module_init

# function _dp_listcomp_3(_dp_iter_2):
#     display_name: <listcomp>
#     block start:
#         _dp_tmp_1 = []
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
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_listcomp_3_1
#                         block _dp_bb__dp_listcomp_3_1:
#                             _dp_tmp_1.append(i)
#                             jump _dp_bb__dp_listcomp_3_3

# function _dp_module_init():
#     block start:
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "_dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "x", _dp_listcomp_3(it))
#         return

# set_comp

x = {i for i in it}

# ==

# module_init: _dp_module_init

# function _dp_setcomp_3(_dp_iter_2):
#     display_name: <setcomp>
#     block start:
#         _dp_tmp_1 = set()
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_setcomp_3_3
#         block _dp_bb__dp_setcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_setcomp_3_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_setcomp_3_2:
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_setcomp_3_1
#                         block _dp_bb__dp_setcomp_3_1:
#                             _dp_tmp_1.add(i)
#                             jump _dp_bb__dp_setcomp_3_3

# function _dp_module_init():
#     block start:
#         _dp_setcomp_3 = __dp_make_function("start", 0, "<setcomp>", "_dp_setcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "x", _dp_setcomp_3(it))
#         return

# dict_comp

x = {k: v for k, v in it}

# ==

# module_init: _dp_module_init

# function _dp_dictcomp_3(_dp_iter_2):
#     display_name: <dictcomp>
#     block start:
#         _dp_tmp_1 = {}
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_dictcomp_3_3
#         block _dp_bb__dp_dictcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_dictcomp_3_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_dictcomp_3_2:
#                         _dp_tmp_4 = __dp_unpack(_dp_tmp_2, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_4, 0)
#                         v = __dp_getitem(_dp_tmp_4, 1)
#                         del _dp_tmp_4
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_dictcomp_3_1
#                         block _dp_bb__dp_dictcomp_3_1:
#                             __dp_setitem(_dp_tmp_1, k, v)
#                             jump _dp_bb__dp_dictcomp_3_3

# function _dp_module_init():
#     block start:
#         _dp_dictcomp_3 = __dp_make_function("start", 0, "<dictcomp>", "_dp_dictcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", None, __dp__.NO_DEFAULT)), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "x", _dp_dictcomp_3(it))
#         return

# attribute_non_chain

x = f().y

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", f().y)
#         return

# fstring_simple

x = f"{a}"

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", f"{a}")
#         return

# tstring_simple

x = t"{a}"

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", t"{a}")
#         return

# complex_literal

x = 1j

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", 1j)
#         return

# float_literal_long

x = 1.234567890123456789

# ==

# module_init: _dp_module_init

# function _dp_module_init():
#     block start:
#         __dp_store_global(globals(), "x", 1.2345678901234567)
#         return
