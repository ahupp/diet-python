# subscript

x = a[b]

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# subscript_slice

x = a[1:2:3]

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# binary_add

x = a + b

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# binary_bitwise_or

x = a | b

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# unary_neg

x = -a

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# boolop_chain

x = a and b or c

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# compare_lt

x = a < b

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# compare_chain

x = a < b < c

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# compare_not_in

x = a not in b

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# if_expr

x = a if cond else b

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# named_expr

x = (y := f())

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# lambda_simple

x = lambda y: y + 1

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# generator_expr

x = (i for i in it)

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# list_literal

x = [a, b]

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# list_literal_splat

x = [a, *b]

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# tuple_splat

x = (a, *b)

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# set_literal

x = {a, b}

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# dict_literal

x = {"a": 1, "b": 2}

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# list_comp

x = [i for i in it]

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# set_comp

x = {i for i in it}

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# dict_comp

x = {k: v for k, v in it}

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# attribute_non_chain

x = f().y

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# fstring_simple

x = f"{a}"

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# tstring_simple

x = t"{a}"

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# complex_literal

x = 1j

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)

# float_literal_long

x = 1.234567890123456789

# ==

__dp_store_global(
    globals(),
    __dp_decode_literal_bytes(b"_dp_module_init"),
    __dp_def_fn(
        __dp_decode_literal_bytes(b"_dp_bb__dp_module_init_start"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_decode_literal_bytes(b"_dp_module_init"),
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
