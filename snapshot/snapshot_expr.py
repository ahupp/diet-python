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
#         _dp_lambda_1 = __dp_make_function("start", 0, "<lambda>", "<lambda>", __dp_tuple("y"), __dp_tuple(__dp_tuple("y", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
#         __dp_store_global(globals(), "x", _dp_lambda_1)
#         return

# generator_expr

x = (i for i in it)

# ==

# module_init: _dp_module_init

# function <genexpr>(_dp_iter_2):
#     display_name: <genexpr>
#     block start:
#         _dp_iter_3 = _dp_iter_2
#         jump _dp_bb_3
#         block _dp_bb_3:
#             jump _dp_bb_2
#             block _dp_bb_2:
#                 _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
#                 if_term __dp_is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
#                     then:
#                         block _dp_bb_0:
#                             return
#                     else:
#                         block _dp_bb_1:
#                             i = _dp_tmp_4
#                             return i

# function _dp_module_init():
#     block start:
#         _dp_genexpr_1 = __dp_make_function("start", 0, "<genexpr>", "<genexpr>", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
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
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_1.append(i)
#                             jump _dp_bb_3

# function _dp_module_init():
#     block start:
#         _dp_listcomp_3 = __dp_make_function("start", 0, "<listcomp>", "_dp_listcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
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
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             _dp_tmp_1.add(i)
#                             jump _dp_bb_3

# function _dp_module_init():
#     block start:
#         _dp_setcomp_3 = __dp_make_function("start", 0, "<setcomp>", "_dp_setcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
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
#         jump _dp_bb_3
#         block _dp_bb_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb_0:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb_2:
#                         _dp_tmp_4 = __dp_unpack(_dp_tmp_2, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_4, 0)
#                         v = __dp_getitem(_dp_tmp_4, 1)
#                         del _dp_tmp_4
#                         _dp_tmp_2 = None
#                         jump _dp_bb_1
#                         block _dp_bb_1:
#                             __dp_setitem(_dp_tmp_1, k, v)
#                             jump _dp_bb_3

# function _dp_module_init():
#     block start:
#         _dp_dictcomp_3 = __dp_make_function("start", 0, "<dictcomp>", "_dp_dictcomp_3", __dp_tuple("_dp_iter_2"), __dp_tuple(__dp_tuple("_dp_iter_2", "Any", False)), __dp_tuple(), __dp_globals(), __name__, None, None)
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
