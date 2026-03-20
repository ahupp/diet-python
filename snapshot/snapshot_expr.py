# subscript

x = a[b]

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# subscript_slice

x = a[1:2:3]

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# binary_add

x = a + b

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# binary_bitwise_or

x = a | b

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# unary_neg

x = -a

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# boolop_chain

x = a and b or c

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# compare_lt

x = a < b

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# compare_chain

x = a < b < c

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# compare_not_in

x = a not in b

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# if_expr

x = a if cond else b

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# named_expr

x = (y := f())

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# lambda_simple

x = lambda y: y + 1

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# generator_expr

x = (i for i in it)

# ==

# snapshot regeneration failed
# panic: core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for <genexpr>

# list_literal

x = [a, b]

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# list_literal_splat

x = [a, *b]

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# tuple_splat

x = (a, *b)

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# set_literal

x = {a, b}

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# dict_literal

x = {"a": 1, "b": 2}

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# list_comp

x = [i for i in it]

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# set_comp

x = {i for i in it}

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# dict_comp

x = {k: v for k, v in it}

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# attribute_non_chain

x = f().y

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# fstring_simple

x = f"{a}"

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# tstring_simple

x = t"{a}"

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# complex_literal

x = 1j

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# float_literal_long

x = 1.234567890123456789

# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked
