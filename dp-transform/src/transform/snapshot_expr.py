# subscript

x = a[b]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_getitem(a, b))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_getitem(a, __dp_slice(1, 2, 3))
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_add(a, b))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_or_(a, b))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_neg(a))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    _dp_target_1 = a
    if _dp_target_1:
        _dp_target_1 = b
    if __dp_not_(_dp_target_1):
        _dp_target_1 = c
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_target_1)


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_lt(a, b))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    _dp_compare_2 = a
    _dp_compare_3 = b
    _dp_target_1 = __dp_lt(_dp_compare_2, _dp_compare_3)
    if _dp_target_1:
        _dp_target_1 = __dp_lt(_dp_compare_3, c)
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_target_1)


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_not_(__dp_contains(b, a))
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    if cond:
        _dp_tmp_1 = a
    else:
        _dp_tmp_1 = b
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_tmp_1)


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"y"), f())
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_load_global(globals(), __dp_decode_literal_bytes(b"y")),
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():

    def _dp_lambda_1(y):
        return __dp_add(y, 1)

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_lambda_1)


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():

    def _dp_genexpr_1(_dp_iter_2):
        _dp_iter_3 = _dp_iter_2
        while True:
            _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
            if __dp_is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
                break
            else:
                i = _dp_tmp_4
                yield i

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), _dp_genexpr_1(__dp_iter(it))
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_list((a, b)))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_list(__dp_add((a,), __dp_tuple(b))),
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_add((a,), __dp_tuple(b))
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_set((a, b)))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_dict(
            ((__dp_decode_literal_bytes(b"a"), 1), (__dp_decode_literal_bytes(b"b"), 2))
        ),
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_or_(
            __dp_or_(__dp_dict(((__dp_decode_literal_bytes(b"a"), 1),)), __dp_dict(m)),
            __dp_dict(((__dp_decode_literal_bytes(b"b"), 2),)),
        ),
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():

    def _dp_listcomp_3(_dp_iter_2):
        _dp_tmp_1 = __dp_list(())
        _dp_iter_4 = __dp_iter(_dp_iter_2)
        while True:
            _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
            if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
                break
            else:
                i = _dp_tmp_5
                _dp_tmp_5 = None
                _dp_tmp_1.append(i)
        return _dp_tmp_1

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_listcomp_3(it))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():

    def _dp_setcomp_3(_dp_iter_2):
        _dp_tmp_1 = set()
        _dp_iter_4 = __dp_iter(_dp_iter_2)
        while True:
            _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
            if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
                break
            else:
                i = _dp_tmp_5
                _dp_tmp_5 = None
                _dp_tmp_1.add(i)
        return _dp_tmp_1

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_setcomp_3(it))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():

    def _dp_dictcomp_3(_dp_iter_2):
        _dp_tmp_1 = __dp_dict()
        _dp_iter_4 = __dp_iter(_dp_iter_2)
        while True:
            _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
            if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
                break
            else:
                _dp_tmp_7 = _dp_tmp_5
                k = __dp_getitem(_dp_tmp_7, 0)
                v = __dp_getitem(_dp_tmp_7, 1)
                del _dp_tmp_7
                _dp_tmp_5 = None
                __dp_setitem(_dp_tmp_1, k, v)
        return _dp_tmp_1

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_dictcomp_3(it))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), f().y)


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_format(a))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        _dp_templatelib.Template(
            *(
                _dp_templatelib.Interpolation(
                    a,
                    __dp_decode_literal_bytes(b"a"),
                    None,
                    __dp_decode_literal_bytes(b""),
                ),
            )
        ),
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), complex(0.0, 1.0))


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
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


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_float_from_literal(__dp_decode_literal_bytes(b"1.234567890123456789")),
    )


# -- bb --
__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        "_dp_bb__dp_module_init_start",
        "_dp_module_init",
        "_dp_module_init",
        __dp_tuple(),
        __dp_tuple(),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
