# subscript

x = a[b]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.getitem(a, b))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.getitem(a, b))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# subscript_slice

x = a[1:2:3]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.getitem(a, __dp__.slice(1, 2, 3)))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.getitem(a, __dp__.slice(1, 2, 3)))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# binary_add

x = a + b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.add(a, b))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.add(a, b))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# binary_bitwise_or

x = a | b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.or_(a, b))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.or_(a, b))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# unary_neg

x = -a

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.neg(a))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.neg(a))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# boolop_chain

x = a and b or c

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_target_1 = a
    if _dp_target_1:
        _dp_target_1 = b
    if __dp__.not_(_dp_target_1):
        _dp_target_1 = c
    __dp__.store_global(globals(), "x", _dp_target_1)


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    _dp_target_1 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.store_global(globals(), "x", _dp_target_1)
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    _dp_target_1 = c
    return __dp__.jump(_dp_bb__dp_module_init_0, (_dp_target_1,))


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_target_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.brif(
        __dp__.not_(_dp_target_1),
        _dp_bb__dp_module_init_1,
        (),
        _dp_bb__dp_module_init_0,
        (_dp_target_1,),
    )


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    _dp_target_1 = b
    return __dp__.jump(_dp_bb__dp_module_init_2, (_dp_target_1,))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_target_1 = a
    return __dp__.brif(
        _dp_target_1,
        _dp_bb__dp_module_init_3,
        (),
        _dp_bb__dp_module_init_2,
        (_dp_target_1,),
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# compare_lt

x = a < b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.lt(a, b))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.lt(a, b))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# compare_chain

x = a < b < c

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_compare_2 = a
    _dp_compare_3 = b
    _dp_target_1 = __dp__.lt(_dp_compare_2, _dp_compare_3)
    if _dp_target_1:
        _dp_target_1 = __dp__.lt(_dp_compare_3, c)
    __dp__.store_global(globals(), "x", _dp_target_1)


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    _dp_target_1 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.store_global(globals(), "x", _dp_target_1)
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    _dp_compare_3 = __dp__.take_arg1(_dp_args_ptr)
    _dp_target_1 = __dp__.lt(_dp_compare_3, c)
    return __dp__.jump(_dp_bb__dp_module_init_0, (_dp_target_1,))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_compare_2 = a
    _dp_compare_3 = b
    _dp_target_1 = __dp__.lt(_dp_compare_2, _dp_compare_3)
    return __dp__.brif(
        _dp_target_1,
        _dp_bb__dp_module_init_1,
        (_dp_compare_3,),
        _dp_bb__dp_module_init_0,
        (_dp_target_1,),
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# compare_not_in

x = a not in b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.not_(__dp__.contains(b, a)))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.not_(__dp__.contains(b, a)))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# if_expr

x = a if cond else b

# ==


# -- pre-bb --
def _dp_module_init():
    if cond:
        _dp_tmp_1 = a
    else:
        _dp_tmp_1 = b
    __dp__.store_global(globals(), "x", _dp_tmp_1)


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.store_global(globals(), "x", _dp_tmp_1)
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    _dp_tmp_1 = a
    return __dp__.jump(_dp_bb__dp_module_init_0, (_dp_tmp_1,))


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_tmp_1 = b
    return __dp__.jump(_dp_bb__dp_module_init_0, (_dp_tmp_1,))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.brif(cond, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_2, ())


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# named_expr

x = (y := f())

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "y", f())
    __dp__.store_global(globals(), "x", __dp__.load_global(globals(), "y"))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "y", f())
    __dp__.store_global(globals(), "x", __dp__.load_global(globals(), "y"))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# lambda_simple

x = lambda y: y + 1

# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_lambda_1(y):
        return __dp__.add(y, 1)

    __dp__.store_global(globals(), "x", _dp_lambda_1)


# -- bb --
def _dp_bb__dp_lambda_1_start(_dp_args_ptr):
    y = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(__dp__.add(y, 1))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_lambda_1 = __dp__.def_fn(
        _dp_bb__dp_lambda_1_start,
        "<lambda>",
        "<lambda>",
        ("y",),
        (("y", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "x", _dp_lambda_1)
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# generator_expr

x = (i for i in it)

# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_genexpr_1(_dp_iter_2):
        _dp_iter_3 = _dp_iter_2
        while True:
            _dp_tmp_4 = __dp__.next_or_sentinel(_dp_iter_3)
            if __dp__.is_(_dp_tmp_4, __dp__.ITER_COMPLETE):
                break
            else:
                i = _dp_tmp_4
                yield i

    __dp__.store_global(globals(), "x", _dp_genexpr_1(__dp__.iter(it)))


# -- bb --
def _dp_bb__dp_genexpr_1_done(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb__dp_genexpr_1_invalid(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    return __dp__.raise_(
        RuntimeError("invalid generator pc: {}".format(_dp_state["pc"]))
    )


def _dp_bb__dp_genexpr_1_internal_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb__dp_genexpr_1_internal_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_tmp_4, _dp_iter_3 = __dp__.take_args(
        _dp_args_ptr
    )
    i = _dp_tmp_4
    __dp__.setitem(_dp_state, "pc", 6)
    __dp__.setitem(_dp_state, "args", (_dp_state, _dp_iter_3))
    return __dp__.ret(i)


def _dp_bb__dp_genexpr_1_internal_2(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_3 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_tmp_4 = __dp__.next_or_sentinel(_dp_iter_3)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_4, __dp__.ITER_COMPLETE),
        _dp_bb__dp_genexpr_1_internal_0,
        (_dp_state, _dp_send_value, _dp_resume_exc),
        _dp_bb__dp_genexpr_1_internal_1,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_tmp_4, _dp_iter_3),
    )


def _dp_bb__dp_genexpr_1_resume_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_3 = __dp__.take_args(
        _dp_args_ptr
    )
    return __dp__.jump(
        _dp_bb__dp_genexpr_1_internal_2,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_3),
    )


def _dp_bb__dp_genexpr_1_resume_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_2 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_iter_3 = _dp_iter_2
    return __dp__.jump(
        _dp_bb__dp_genexpr_1_resume_0,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_3),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_genexpr_1 = __dp__.def_gen(
        6,
        (
            _dp_bb__dp_genexpr_1_done,
            _dp_bb__dp_genexpr_1_invalid,
            _dp_bb__dp_genexpr_1_internal_0,
            _dp_bb__dp_genexpr_1_internal_1,
            _dp_bb__dp_genexpr_1_internal_2,
            _dp_bb__dp_genexpr_1_resume_0,
            _dp_bb__dp_genexpr_1_resume_1,
        ),
        (-1, -1, -1, -1, -1, -1, -1),
        "<genexpr>",
        "<genexpr>",
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "x", _dp_genexpr_1(__dp__.iter(it)))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# list_literal

x = [a, b]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.list((a, b)))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.list((a, b)))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# list_literal_splat

x = [a, *b]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.list(__dp__.add((a,), __dp__.tuple(b))))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.list(__dp__.add((a,), __dp__.tuple(b))))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# tuple_splat

x = (a, *b)

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.add((a,), __dp__.tuple(b)))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.add((a,), __dp__.tuple(b)))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# set_literal

x = {a, b}

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.set((a, b)))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.set((a, b)))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# dict_literal

x = {"a": 1, "b": 2}

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.dict((("a", 1), ("b", 2))))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.dict((("a", 1), ("b", 2))))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# dict_literal_unpack

x = {"a": 1, **m, "b": 2}

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(
        globals(),
        "x",
        __dp__.or_(
            __dp__.or_(__dp__.dict((("a", 1),)), __dp__.dict(m)),
            __dp__.dict((("b", 2),)),
        ),
    )


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "x",
        __dp__.or_(
            __dp__.or_(__dp__.dict((("a", 1),)), __dp__.dict(m)),
            __dp__.dict((("b", 2),)),
        ),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# list_comp

x = [i for i in it]

# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_listcomp_3(_dp_iter_2):
        _dp_tmp_1 = __dp__.list(())
        for i in _dp_iter_2:
            _dp_tmp_1.append(i)
        return _dp_tmp_1

    __dp__.store_global(globals(), "x", _dp_listcomp_3(it))


# -- bb --
def _dp_bb__dp_listcomp_3_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, i = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_1.append(i)
    return __dp__.jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = __dp__.take_args(_dp_args_ptr)
    i = _dp_tmp_3
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_listcomp_3_1, (_dp_iter_2, _dp_tmp_1, i))


def _dp_bb__dp_listcomp_3_3(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_3 = __dp__.next_or_sentinel(_dp_iter_2)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_3, __dp__.ITER_COMPLETE),
        _dp_bb__dp_listcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_listcomp_3_2,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_listcomp_3_start(_dp_args_ptr):
    _dp_iter_2 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_1 = __dp__.list(())
    _dp_iter_2 = __dp__.iter(_dp_iter_2)
    return __dp__.jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_listcomp_3 = __dp__.def_fn(
        _dp_bb__dp_listcomp_3_start,
        "<listcomp>",
        "_dp_listcomp_3",
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "x", _dp_listcomp_3(it))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# set_comp

x = {i for i in it}

# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_setcomp_3(_dp_iter_2):
        _dp_tmp_1 = set()
        for i in _dp_iter_2:
            _dp_tmp_1.add(i)
        return _dp_tmp_1

    __dp__.store_global(globals(), "x", _dp_setcomp_3(it))


# -- bb --
def _dp_bb__dp_setcomp_3_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_1)


def _dp_bb__dp_setcomp_3_1(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, i = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_1.add(i)
    return __dp__.jump(_dp_bb__dp_setcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_setcomp_3_2(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = __dp__.take_args(_dp_args_ptr)
    i = _dp_tmp_3
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_setcomp_3_1, (_dp_iter_2, _dp_tmp_1, i))


def _dp_bb__dp_setcomp_3_3(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_3 = __dp__.next_or_sentinel(_dp_iter_2)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_3, __dp__.ITER_COMPLETE),
        _dp_bb__dp_setcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_setcomp_3_2,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_setcomp_3_start(_dp_args_ptr):
    _dp_iter_2 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_1 = set()
    _dp_iter_2 = __dp__.iter(_dp_iter_2)
    return __dp__.jump(_dp_bb__dp_setcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_setcomp_3 = __dp__.def_fn(
        _dp_bb__dp_setcomp_3_start,
        "<setcomp>",
        "_dp_setcomp_3",
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "x", _dp_setcomp_3(it))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# dict_comp

x = {k: v for k, v in it}

# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_dictcomp_3(_dp_iter_2):
        _dp_tmp_1 = __dp__.dict()
        for k, v in _dp_iter_2:
            __dp__.setitem(_dp_tmp_1, k, v)
        return _dp_tmp_1

    __dp__.store_global(globals(), "x", _dp_dictcomp_3(it))


# -- bb --
def _dp_bb__dp_dictcomp_3_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_1)


def _dp_bb__dp_dictcomp_3_1(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, k, v = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_tmp_1, k, v)
    return __dp__.jump(_dp_bb__dp_dictcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_dictcomp_3_2(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = __dp__.take_args(_dp_args_ptr)
    k = _dp_tmp_3[0]
    v = _dp_tmp_3[1]
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_dictcomp_3_1, (_dp_iter_2, _dp_tmp_1, k, v))


def _dp_bb__dp_dictcomp_3_3(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_3 = __dp__.next_or_sentinel(_dp_iter_2)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_3, __dp__.ITER_COMPLETE),
        _dp_bb__dp_dictcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_dictcomp_3_2,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_dictcomp_3_start(_dp_args_ptr):
    _dp_iter_2 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_1 = __dp__.dict()
    _dp_iter_2 = __dp__.iter(_dp_iter_2)
    return __dp__.jump(_dp_bb__dp_dictcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_dictcomp_3 = __dp__.def_fn(
        _dp_bb__dp_dictcomp_3_start,
        "<dictcomp>",
        "_dp_dictcomp_3",
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "x", _dp_dictcomp_3(it))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# attribute_non_chain

x = f().y

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", f().y)


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", f().y)
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# fstring_simple

x = f"{a}"

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", __dp__.builtins.format(a))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", __dp__.builtins.format(a))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# tstring_simple

x = t"{a}"

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(
        globals(),
        "x",
        _dp_templatelib.Template(*(_dp_templatelib.Interpolation(a, "a", None, ""),)),
    )


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "x",
        _dp_templatelib.Template(*(_dp_templatelib.Interpolation(a, "a", None, ""),)),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# complex_literal

x = 1j

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", complex(0.0, 1.0))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", complex(0.0, 1.0))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# float_literal_long

x = 1.234567890123456789

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(
        globals(), "x", __dp__.float_from_literal("1.234567890123456789")
    )


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(), "x", __dp__.float_from_literal("1.234567890123456789")
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start
