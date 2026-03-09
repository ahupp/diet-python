# subscript

x = a[b]

# ==


# subscript_slice

x = a[1:2:3]

# ==


# binary_add

x = a + b

# ==


# binary_bitwise_or

x = a | b

# ==


# unary_neg

x = -a

# ==


# boolop_chain

x = a and b or c

# ==


# compare_lt

x = a < b

# ==


# compare_chain

x = a < b < c

# ==


# compare_not_in

x = a not in b

# ==


# if_expr

x = a if cond else b

# ==


# named_expr

x = (y := f())

# ==


# lambda_simple

x = lambda y: y + 1

# ==


# generator_expr

x = (i for i in it)

# ==


# list_literal

x = [a, b]

# ==


# list_literal_splat

x = [a, *b]

# ==


# tuple_splat

x = (a, *b)

# ==


# set_literal

x = {a, b}

# ==


# dict_literal

x = {"a": 1, "b": 2}

# ==


# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==


# list_comp

x = [i for i in it]

# ==


# set_comp

x = {i for i in it}

# ==


# dict_comp

x = {k: v for k, v in it}

# ==


# attribute_non_chain

x = f().y

# ==


# fstring_simple

x = f"{a}"

# ==


# tstring_simple

x = t"{a}"

# ==


# complex_literal

x = 1j

# ==


# float_literal_long

x = 1.234567890123456789

# ==
