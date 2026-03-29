# subscript

x = a[b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = a[b]
#         return __dp_NONE

# subscript_slice

x = a[1:2:3]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = a[1:2:3]
#         return __dp_NONE

# binary_add

x = a + b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = a + b
#         return __dp_NONE

# binary_bitwise_or

x = a | b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = a | b
#         return __dp_NONE

# unary_neg

x = -a

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = -a
#         return __dp_NONE

# boolop_chain

x = a and b or c

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         _dp_target_2 = a
#         if_term _dp_target_2:
#             then:
#                 block bb2:
#                     params: [_dp_target_2:Local]
#                     _dp_target_2 = b
#                     jump bb4
#             else:
#                 block bb3:
#                     params: [_dp_target_2:Local]
#                     jump bb4
#         block bb4:
#             params: [_dp_target_2:Local]
#             _dp_target_1 = _dp_target_2
#             if_term not _dp_target_1:
#                 then:
#                     block bb5:
#                         params: [_dp_target_2:Local, _dp_target_1:Local]
#                         _dp_target_1 = c
#                         jump bb7
#                 else:
#                     block bb6:
#                         params: [_dp_target_2:Local, _dp_target_1:Local]
#                         jump bb7
#             block bb7:
#                 params: [_dp_target_2:Local, _dp_target_1:Local]
#                 x = _dp_target_1
#                 return __dp_NONE

# compare_lt

x = a < b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = __dp_lt(a, b)
#         return __dp_NONE

# compare_chain

x = a < b < c

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         _dp_compare_1 = a
#         _dp_compare_3 = b
#         _dp_target_2 = __dp_lt(_dp_compare_1, _dp_compare_3)
#         if_term _dp_target_2:
#             then:
#                 block bb2:
#                     params: [_dp_compare_1:Local, _dp_compare_3:Local, _dp_target_2:Local]
#                     _dp_target_2 = __dp_lt(_dp_compare_3, c)
#                     jump bb4
#             else:
#                 block bb3:
#                     params: [_dp_compare_1:Local, _dp_compare_3:Local, _dp_target_2:Local]
#                     jump bb4
#         block bb4:
#             params: [_dp_compare_1:Local, _dp_compare_3:Local, _dp_target_2:Local]
#             x = _dp_target_2
#             return __dp_NONE

# compare_not_in

x = a not in b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = __dp_not_(__dp_contains(b, a))
#         return __dp_NONE

# if_expr

x = a if cond else b

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         if_term cond:
#             then:
#                 block bb2:
#                     _dp_tmp_1 = a
#                     jump bb4
#             else:
#                 block bb3:
#                     _dp_tmp_1 = b
#                     jump bb4
#         block bb4:
#             params: [_dp_tmp_1:Local]
#             x = _dp_tmp_1
#             return __dp_NONE

# named_expr

x = (y := f())

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         y = f()
#         x = y
#         return __dp_NONE

# lambda_simple

x = lambda y: y + 1

# ==

# function <lambda>(y):
#     function_id: 0
#     display_name: <lambda>
#     block bb1:
#         params: [y:Local]
#         return y + 1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         _dp_lambda_1 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         x = _dp_lambda_1
#         return __dp_NONE

# generator_expr

x = (i for i in it)

# ==

# generator <genexpr>(_dp_iter_2):
#     function_id: 0
#     display_name: <genexpr>
#     block bb2:
#         params: [_dp_iter_2:Local]
#         _dp_iter_3 = _dp_iter_2
#         jump bb1
#         block bb1:
#             params: [_dp_iter_3:Local]
#             jump bb3
#             block bb3:
#                 params: [_dp_iter_3:Local]
#                 _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
#                 if_term __dp_is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
#                     then:
#                         block bb4:
#                             return __dp_NONE
#                     else:
#                         block bb5:
#                             params: [_dp_iter_3:Local, _dp_tmp_4:Local]
#                             i = _dp_tmp_4
#                             yield i
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         _dp_genexpr_1 = __dp_make_function(0, "generator", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         x = _dp_genexpr_1(__dp_iter(it))
#         return __dp_NONE

# list_literal

x = [a, b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = [a, b]
#         return __dp_NONE

# list_literal_splat

x = [a, *b]

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = [a, *b]
#         return __dp_NONE

# tuple_splat

x = (a, *b)

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = a, *b
#         return __dp_NONE

# set_literal

x = {a, b}

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = {a, b}
#         return __dp_NONE

# dict_literal

x = {"a": 1, "b": 2}

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = {"a": 1, "b": 2}
#         return __dp_NONE

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = {"a": 1, **m, "b": 2}
#         return __dp_NONE

# list_comp

x = [i for i in it]

# ==

# function _dp_listcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <listcomp>
#     block bb3:
#         params: [_dp_iter_2:Local]
#         _dp_tmp_1 = []
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump bb1
#         block bb1:
#             params: [_dp_tmp_1:Local, _dp_iter_0_0:Local]
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         params: [_dp_tmp_1:Local]
#                         return _dp_tmp_1
#                 else:
#                     block bb2:
#                         params: [_dp_tmp_1:Local, _dp_iter_0_0:Local, _dp_tmp_0_1:Local]
#                         i = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             params: [_dp_tmp_1:Local, _dp_iter_0_0:Local, i:Local]
#                             _dp_tmp_1.append(i)
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         _dp_listcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         x = _dp_listcomp_3(it)
#         return __dp_NONE

# set_comp

x = {i for i in it}

# ==

# function _dp_setcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <setcomp>
#     block bb3:
#         params: [_dp_iter_2:Local]
#         _dp_tmp_1 = set()
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump bb1
#         block bb1:
#             params: [_dp_tmp_1:Local, _dp_iter_0_0:Local]
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         params: [_dp_tmp_1:Local]
#                         return _dp_tmp_1
#                 else:
#                     block bb2:
#                         params: [_dp_tmp_1:Local, _dp_iter_0_0:Local, _dp_tmp_0_1:Local]
#                         i = _dp_tmp_0_1
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             params: [_dp_tmp_1:Local, _dp_iter_0_0:Local, i:Local]
#                             _dp_tmp_1.add(i)
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         _dp_setcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         x = _dp_setcomp_3(it)
#         return __dp_NONE

# dict_comp

x = {k: v for k, v in it}

# ==

# function _dp_dictcomp_3(_dp_iter_2):
#     function_id: 0
#     display_name: <dictcomp>
#     block bb3:
#         params: [_dp_iter_2:Local]
#         _dp_tmp_1 = {}
#         _dp_iter_0_0 = __dp_iter(_dp_iter_2)
#         jump bb1
#         block bb1:
#             params: [_dp_tmp_1:Local, _dp_iter_0_0:Local]
#             _dp_tmp_0_1 = __dp_next_or_sentinel(_dp_iter_0_0)
#             if_term __dp_is_(_dp_tmp_0_1, __dp__.ITER_COMPLETE):
#                 then:
#                     block bb4:
#                         params: [_dp_tmp_1:Local]
#                         return _dp_tmp_1
#                 else:
#                     block bb2:
#                         params: [_dp_tmp_1:Local, _dp_iter_0_0:Local, _dp_tmp_0_1:Local]
#                         _dp_tmp_0_2 = __dp_unpack(_dp_tmp_0_1, __dp_tuple(True, True))
#                         k = __dp_getitem(_dp_tmp_0_2, 0)
#                         v = __dp_getitem(_dp_tmp_0_2, 1)
#                         del _dp_tmp_0_2
#                         _dp_tmp_0_1 = None
#                         jump bb5
#                         block bb5:
#                             params: [_dp_tmp_1:Local, _dp_iter_0_0:Local, k:Local, v:Local]
#                             __dp_setitem(_dp_tmp_1, k, v)
#                             jump bb1

# function _dp_module_init():
#     function_id: 1
#     block bb1:
#         _dp_dictcomp_3 = __dp_make_function(0, "function", __dp_tuple(), __dp_tuple(), __dp_globals(), None)
#         x = _dp_dictcomp_3(it)
#         return __dp_NONE

# attribute_non_chain

x = f().y

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = f().y
#         return __dp_NONE

# fstring_simple

x = f"{a}"

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = f"{a}"
#         return __dp_NONE

# tstring_simple

x = t"{a}"

# ==

# function _dp_module_init():
#     function_id: 0
#     block bb1:
#         x = t"{a}"
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
#     block bb1:
#         x = 1.2345678901234567
#         return __dp_NONE
