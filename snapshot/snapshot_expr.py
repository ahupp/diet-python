# subscript

x = a[b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", a[b])
#         return __dp_NONE

# subscript_slice

x = a[1:2:3]

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", a[1:2:3])
#         return __dp_NONE

# binary_add

x = a + b

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", a + b)
#         return __dp_NONE

# binary_bitwise_or

x = a | b

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", a | b)
#         return __dp_NONE

# unary_neg

x = -a

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", -a)
#         return __dp_NONE

# boolop_chain

x = a and b or c

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         _dp_target_2 = a
#         if _dp_target_2:
#             _dp_target_2 = b
#         _dp_target_1 = _dp_target_2
#         if not _dp_target_1:
#             _dp_target_1 = c
#         __dp_store_global(globals(), "x", _dp_target_1)
#         return __dp_NONE

# compare_lt

x = a < b

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", __dp_lt(a, b))
#         return __dp_NONE

# compare_chain

x = a < b < c

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         _dp_compare_1 = a
#         _dp_compare_3 = b
#         _dp_target_2 = __dp_lt(_dp_compare_1, _dp_compare_3)
#         if _dp_target_2:
#             _dp_target_2 = __dp_lt(_dp_compare_3, c)
#         __dp_store_global(globals(), "x", _dp_target_2)
#         return __dp_NONE

# compare_not_in

x = a not in b

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", __dp_not_(__dp_contains(b, a)))
#         return __dp_NONE

# if_expr

x = a if cond else b

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         if cond:
#             _dp_tmp_1 = a
#         else:
#             _dp_tmp_1 = b
#         __dp_store_global(globals(), "x", _dp_tmp_1)
#         return __dp_NONE

# named_expr

x = (y := f())

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", __dp_store_global(globals(), "y", f()))
#         return __dp_NONE

# lambda_simple

x = lambda y: y + 1

# ==

# function <lambda>(y):
#     function_id: 0
#     display_name: <lambda>
#     block _dp_bb__dp_lambda_1_1:
#         return y + 1

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb__dp_module_init_3:
#         _dp_lambda_1 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "x", _dp_lambda_1)
#         return __dp_NONE

# generator_expr

x = (i for i in it)

# ==

# generator <genexpr>(_dp_iter_2):
#     function_id: 0
#     display_name: <genexpr>
#     block _dp_bb__dp_genexpr_1_2:
#         _dp_iter_3 = _dp_iter_2
#         jump _dp_bb__dp_genexpr_1_1
#         block _dp_bb__dp_genexpr_1_1:
#             jump _dp_bb__dp_genexpr_1_3
#             block _dp_bb__dp_genexpr_1_3:
#                 _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
#                 if_term __dp_is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
#                     then:
#                         block _dp_bb__dp_genexpr_1_4:
#                             return __dp_NONE
#                     else:
#                         block _dp_bb__dp_genexpr_1_5:
#                             i = _dp_tmp_4
#                             yield i
#                             jump _dp_bb__dp_genexpr_1_1

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb__dp_module_init_7:
#         _dp_genexpr_1 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "x", _dp_genexpr_1(__dp_iter(it)))
#         return __dp_NONE

# list_literal

x = [a, b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", [a, b])
#         return __dp_NONE

# list_literal_splat

x = [a, *b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", [a, *b])
#         return __dp_NONE

# tuple_splat

x = (a, *b)

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", (a, *b))
#         return __dp_NONE

# set_literal

x = {a, b}

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", {a, b})
#         return __dp_NONE

# dict_literal

x = {"a": 1, "b": 2}

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", {"a": 1, "b": 2})
#         return __dp_NONE

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", {"a": 1, **m, "b": 2})
#         return __dp_NONE

# list_comp

x = [i for i in it]

# ==

# function _dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block _dp_bb__dp_listcomp_3_5:
#         _dp_tmp_1 = []
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_listcomp_3_3
#         block _dp_bb__dp_listcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_listcomp_3_6:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_listcomp_3_4:
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_listcomp_3_7
#                         block _dp_bb__dp_listcomp_3_7:
#                             _dp_tmp_1.append(i)
#                             jump _dp_bb__dp_listcomp_3_3

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb__dp_module_init_9:
#         _dp_listcomp_3 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "x", _dp_listcomp_3(it))
#         return __dp_NONE

# set_comp

x = {i for i in it}

# ==

# function _dp_setcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <setcomp>
#     block _dp_bb__dp_setcomp_3_5:
#         _dp_tmp_1 = set()
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_setcomp_3_3
#         block _dp_bb__dp_setcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_setcomp_3_6:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_setcomp_3_4:
#                         i = _dp_tmp_2
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_setcomp_3_7
#                         block _dp_bb__dp_setcomp_3_7:
#                             _dp_tmp_1.add(i)
#                             jump _dp_bb__dp_setcomp_3_3

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb__dp_module_init_9:
#         _dp_setcomp_3 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "x", _dp_setcomp_3(it))
#         return __dp_NONE

# dict_comp

x = {k: v for k, v in it}

# ==

# function _dp_dictcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <dictcomp>
#     block _dp_bb__dp_dictcomp_3_6:
#         _dp_tmp_1 = {}
#         _dp_iter_1 = __dp_iter(_dp_iter_2)
#         jump _dp_bb__dp_dictcomp_3_3
#         block _dp_bb__dp_dictcomp_3_3:
#             _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
#             if_term __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
#                 then:
#                     block _dp_bb__dp_dictcomp_3_7:
#                         return _dp_tmp_1
#                 else:
#                     block _dp_bb__dp_dictcomp_3_5:
#                         _dp_tmp_4 = __dp_unpack(_dp_tmp_2, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_4, 0)
#                         v = __dp_getitem(_dp_tmp_4, 1)
#                         del _dp_tmp_4
#                         _dp_tmp_2 = None
#                         jump _dp_bb__dp_dictcomp_3_8
#                         block _dp_bb__dp_dictcomp_3_8:
#                             __dp_setitem(_dp_tmp_1, k, v)
#                             jump _dp_bb__dp_dictcomp_3_3

# function _dp_module_init():
#     function_id: 1
#     block _dp_bb__dp_module_init_10:
#         _dp_dictcomp_3 = __dp_make_function(0, __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         __dp_store_global(globals(), "x", _dp_dictcomp_3(it))
#         return __dp_NONE

# attribute_non_chain

x = f().y

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", f().y)
#         return __dp_NONE

# fstring_simple

x = f"{a}"

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", f"{a}")
#         return __dp_NONE

# tstring_simple

x = t"{a}"

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", t"{a}")
#         return __dp_NONE

# complex_literal

x = 1j

# ==

# snapshot regeneration failed
# panic: complex literal reached late core BlockPy boundary

# float_literal_long

x = 1.234567890123456789

# ==

# function _dp_module_init():
#     function_id: 0
#     block _dp_bb__dp_module_init_1:
#         __dp_store_global(globals(), "x", 1.2345678901234567)
#         return __dp_NONE
