# subscript

x = a[b]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_getitem(a, b))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_getitem(a, b))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# subscript_slice

x = a[1:2:3]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_getitem(a, __dp_slice(1, 2, 3))
    )


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_getitem(a, __dp_slice(1, 2, 3))
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# binary_add

x = a + b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_add(a, b))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_add(a, b))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# binary_bitwise_or

x = a | b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_or_(a, b))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_or_(a, b))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# unary_neg

x = -a

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_neg(a))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_neg(a))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
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
    if __dp_not_(_dp_target_1):
        _dp_target_1 = c
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_target_1)


# -- bb --
def _dp_bb__dp_module_init_0(_dp_target_1):
    _dp_target_1 = _dp_target_1.take()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_target_1)
    return __dp_ret(None)


def _dp_bb__dp_module_init_1():
    _dp_target_1 = c
    return __dp_jump(_dp_bb__dp_module_init_0, (_dp_target_1,))


def _dp_bb__dp_module_init_2(_dp_target_1):
    _dp_target_1 = _dp_target_1.take()
    return __dp_brif(
        __dp_not_(_dp_target_1),
        _dp_bb__dp_module_init_1,
        (),
        _dp_bb__dp_module_init_0,
        (_dp_target_1,),
    )


def _dp_bb__dp_module_init_3():
    _dp_target_1 = b
    return __dp_jump(_dp_bb__dp_module_init_2, (_dp_target_1,))


def _dp_bb__dp_module_init_start():
    _dp_target_1 = a
    return __dp_brif(
        _dp_target_1,
        _dp_bb__dp_module_init_3,
        (),
        _dp_bb__dp_module_init_2,
        (_dp_target_1,),
    )


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# compare_lt

x = a < b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_lt(a, b))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_lt(a, b))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_0(_dp_target_1):
    _dp_target_1 = _dp_target_1.take()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_target_1)
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(_dp_compare_3):
    _dp_compare_3 = _dp_compare_3.take()
    _dp_target_1 = __dp_lt(_dp_compare_3, c)
    return __dp_jump(_dp_bb__dp_module_init_0, (_dp_target_1,))


def _dp_bb__dp_module_init_start():
    _dp_compare_2 = a
    _dp_compare_3 = b
    _dp_target_1 = __dp_lt(_dp_compare_2, _dp_compare_3)
    return __dp_brif(
        _dp_target_1,
        _dp_bb__dp_module_init_1,
        (_dp_compare_3,),
        _dp_bb__dp_module_init_0,
        (_dp_target_1,),
    )


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# compare_not_in

x = a not in b

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_not_(__dp_contains(b, a))
    )


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_not_(__dp_contains(b, a))
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
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
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_tmp_1)


# -- bb --
def _dp_bb__dp_module_init_0(_dp_tmp_1):
    _dp_tmp_1 = _dp_tmp_1.take()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_tmp_1)
    return __dp_ret(None)


def _dp_bb__dp_module_init_1():
    _dp_tmp_1 = a
    return __dp_jump(_dp_bb__dp_module_init_0, (_dp_tmp_1,))


def _dp_bb__dp_module_init_2():
    _dp_tmp_1 = b
    return __dp_jump(_dp_bb__dp_module_init_0, (_dp_tmp_1,))


def _dp_bb__dp_module_init_start():
    return __dp_brif(cond, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_2, ())


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"y"), f())
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_load_global(globals(), __dp_decode_literal_bytes(b"y")),
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# lambda_simple

x = lambda y: y + 1

# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_lambda_1(y):
        return __dp_add(y, 1)

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_lambda_1)


# -- bb --
def _dp_bb__dp_lambda_1_start(y):
    y = y.take()
    return __dp_ret(__dp_add(y, 1))


def _dp_bb__dp_module_init_start():
    _dp_lambda_1 = __dp_def_fn(
        _dp_bb__dp_lambda_1_start,
        __dp_decode_literal_bytes(b"<lambda>"),
        __dp_decode_literal_bytes(b"<lambda>"),
        ("y",),
        (("y", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_lambda_1)
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
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
def _dp_bb__dp_genexpr_1_done(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb__dp_genexpr_1_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(
        RuntimeError(
            __dp_getattr(
                __dp_decode_literal_bytes(b"invalid generator pc: {}"),
                __dp_decode_literal_bytes(b"format"),
            )(__dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")))
        )
    )


def _dp_bb__dp_genexpr_1_uncaught(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_6
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_6 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_6.take(),
    )
    return __dp_brif(
        __dp_ne(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb__dp_genexpr_1_uncaught_set_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_6",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_6"),
            ),
        ),
        _dp_bb__dp_genexpr_1_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_6",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_6"),
            ),
        ),
    )


def _dp_bb__dp_genexpr_1_uncaught_set_done(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_6
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_6 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_6.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_6)
    return __dp_jump(
        _dp_bb__dp_genexpr_1_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_6",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_6"),
            ),
        ),
    )


def _dp_bb__dp_genexpr_1_uncaught_raise(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_6
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_6 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_6.take(),
    )
    return __dp_raise_(_dp_uncaught_exc_6)


def _dp_bb__dp_genexpr_1_internal_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb__dp_genexpr_1_internal_1(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_tmp_4, _dp_iter_3
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_tmp_4, _dp_iter_3 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_tmp_4.take(),
        _dp_iter_3.take(),
    )
    i = _dp_tmp_4
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"_pc"), 1)
    __dp_store_local(_dp_self, __dp_decode_literal_bytes(b"_dp_iter_3"), _dp_iter_3)
    return __dp_ret(i)


def _dp_bb__dp_genexpr_1_internal_2(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_3
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_3 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_3.take(),
    )
    _dp_tmp_4 = __dp_next_or_sentinel(_dp_iter_3)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_4, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb__dp_genexpr_1_internal_0,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb__dp_genexpr_1_internal_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_tmp_4", __dp_load_local_raw(_dp_self, "_dp_tmp_4")),
            locals().get("_dp_iter_3", __dp_load_local_raw(_dp_self, "_dp_iter_3")),
        ),
    )


def _dp_bb__dp_genexpr_1_resume_0(_dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_3):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_3 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_3.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    return __dp_jump(
        _dp_bb__dp_genexpr_1_internal_2,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_3", __dp_load_local_raw(_dp_self, "_dp_iter_3")),
        ),
    )


def _dp_bb__dp_genexpr_1_resume_1(_dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_2.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    _dp_target_5 = __dp_is_(_dp_resume_exc, None)
    if _dp_target_5:
        _dp_target_5 = __dp_is_not(_dp_send_value, None)
    if _dp_target_5:
        return __dp_raise_(
            TypeError("can't send non-None value to a just-started generator")
        )
    _dp_iter_3 = _dp_iter_2
    return __dp_jump(
        _dp_bb__dp_genexpr_1_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_3", __dp_load_local_raw(_dp_self, "_dp_iter_3")),
        ),
    )


def _dp_bb__dp_genexpr_1_dispatch_throw_done(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb__dp_genexpr_1_dispatch_throw_unstarted(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb__dp_genexpr_1_dispatch_send_target_0(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb__dp_genexpr_1_resume_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
        ),
    )


def _dp_bb__dp_genexpr_1_dispatch_throw_target_0(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb__dp_genexpr_1_dispatch_throw_unstarted,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb__dp_genexpr_1_dispatch_send_target_1(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb__dp_genexpr_1_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_3", __dp_load_local_raw(_dp_self, "_dp_iter_3")),
        ),
    )


def _dp_bb__dp_genexpr_1_dispatch_throw_target_1(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb__dp_genexpr_1_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_3", __dp_load_local_raw(_dp_self, "_dp_iter_3")),
        ),
    )


def _dp_bb__dp_genexpr_1_dispatch_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb__dp_genexpr_1_invalid, (_dp_self, _dp_send_value, _dp_resume_exc)
    )


def _dp_bb__dp_genexpr_1_dispatch_send_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (
            _dp_bb__dp_genexpr_1_dispatch_send_target_0,
            _dp_bb__dp_genexpr_1_dispatch_send_target_1,
        ),
        _dp_bb__dp_genexpr_1_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb__dp_genexpr_1_dispatch_throw_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (
            _dp_bb__dp_genexpr_1_dispatch_throw_target_0,
            _dp_bb__dp_genexpr_1_dispatch_throw_target_1,
        ),
        _dp_bb__dp_genexpr_1_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb__dp_genexpr_1_dispatch_send(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_eq(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb__dp_genexpr_1_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb__dp_genexpr_1_dispatch_send_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb__dp_genexpr_1_dispatch_throw(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_eq(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb__dp_genexpr_1_dispatch_throw_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb__dp_genexpr_1_dispatch_throw_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb__dp_genexpr_1_dispatch(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_is_(_dp_resume_exc, __dp_NONE),
        _dp_bb__dp_genexpr_1_dispatch_send,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb__dp_genexpr_1_dispatch_throw,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


__dp_setattr(
    _dp_bb__dp_genexpr_1_internal_0, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_internal_0, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_internal_1, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_internal_1, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_internal_2, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_internal_2, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_resume_0, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_resume_0, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_resume_1, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_resume_1, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send_target_0,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send_target_0, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw_target_0,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw_target_0, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send_target_1,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send_target_1, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw_target_1,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw_target_1, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_invalid,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_invalid, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send_table,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send_table, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw_table,
    "_dp_exc_target",
    _dp_bb__dp_genexpr_1_uncaught,
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw_table, "_dp_exc_name", "_dp_uncaught_exc_6"
)
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_send, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_dispatch_send, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch_throw, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_dispatch_throw, "_dp_exc_name", "_dp_uncaught_exc_6")
__dp_setattr(
    _dp_bb__dp_genexpr_1_dispatch, "_dp_exc_target", _dp_bb__dp_genexpr_1_uncaught
)
__dp_setattr(_dp_bb__dp_genexpr_1_dispatch, "_dp_exc_name", "_dp_uncaught_exc_6")


def _dp_bb__dp_module_init_start():
    _dp_genexpr_1 = __dp_def_gen(
        _dp_bb__dp_genexpr_1_dispatch,
        __dp_decode_literal_bytes(b"<genexpr>"),
        __dp_decode_literal_bytes(b"<genexpr>"),
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), _dp_genexpr_1(__dp_iter(it))
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# list_literal

x = [a, b]

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_list((a, b)))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_list((a, b)))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_list(__dp_add((a,), __dp_tuple(b))),
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# tuple_splat

x = (a, *b)

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_add((a,), __dp_tuple(b))
    )


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_add((a,), __dp_tuple(b))
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# set_literal

x = {a, b}

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_set((a, b)))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_set((a, b)))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_dict(
            ((__dp_decode_literal_bytes(b"a"), 1), (__dp_decode_literal_bytes(b"b"), 2))
        ),
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_or_(
            __dp_or_(__dp_dict(((__dp_decode_literal_bytes(b"a"), 1),)), __dp_dict(m)),
            __dp_dict(((__dp_decode_literal_bytes(b"b"), 2),)),
        ),
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_listcomp_3_0(_dp_tmp_1):
    _dp_tmp_1 = _dp_tmp_1.take()
    return __dp_ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_iter_2, _dp_tmp_1, i):
    _dp_iter_2, _dp_tmp_1, i = _dp_iter_2.take(), _dp_tmp_1.take(), i.take()
    __dp_getattr(_dp_tmp_1, __dp_decode_literal_bytes(b"append"))(i)
    return __dp_jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_iter_2, _dp_tmp_1, _dp_tmp_3):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = (
        _dp_iter_2.take(),
        _dp_tmp_1.take(),
        _dp_tmp_3.take(),
    )
    i = _dp_tmp_3
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(_dp_bb__dp_listcomp_3_1, (_dp_iter_2, _dp_tmp_1, i))


def _dp_bb__dp_listcomp_3_3(_dp_iter_2, _dp_tmp_1):
    _dp_iter_2, _dp_tmp_1 = _dp_iter_2.take(), _dp_tmp_1.take()
    _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_3, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb__dp_listcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_listcomp_3_2,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_listcomp_3_start(_dp_iter_2):
    _dp_iter_2 = _dp_iter_2.take()
    _dp_tmp_1 = __dp_list(())
    _dp_iter_2 = __dp_iter(_dp_iter_2)
    return __dp_jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_module_init_start():
    _dp_listcomp_3 = __dp_def_fn(
        _dp_bb__dp_listcomp_3_start,
        __dp_decode_literal_bytes(b"<listcomp>"),
        __dp_decode_literal_bytes(b"_dp_listcomp_3"),
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_listcomp_3(it))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_setcomp_3_0(_dp_tmp_1):
    _dp_tmp_1 = _dp_tmp_1.take()
    return __dp_ret(_dp_tmp_1)


def _dp_bb__dp_setcomp_3_1(_dp_iter_2, _dp_tmp_1, i):
    _dp_iter_2, _dp_tmp_1, i = _dp_iter_2.take(), _dp_tmp_1.take(), i.take()
    __dp_getattr(_dp_tmp_1, __dp_decode_literal_bytes(b"add"))(i)
    return __dp_jump(_dp_bb__dp_setcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_setcomp_3_2(_dp_iter_2, _dp_tmp_1, _dp_tmp_3):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = (
        _dp_iter_2.take(),
        _dp_tmp_1.take(),
        _dp_tmp_3.take(),
    )
    i = _dp_tmp_3
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(_dp_bb__dp_setcomp_3_1, (_dp_iter_2, _dp_tmp_1, i))


def _dp_bb__dp_setcomp_3_3(_dp_iter_2, _dp_tmp_1):
    _dp_iter_2, _dp_tmp_1 = _dp_iter_2.take(), _dp_tmp_1.take()
    _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_3, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb__dp_setcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_setcomp_3_2,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_setcomp_3_start(_dp_iter_2):
    _dp_iter_2 = _dp_iter_2.take()
    _dp_tmp_1 = set()
    _dp_iter_2 = __dp_iter(_dp_iter_2)
    return __dp_jump(_dp_bb__dp_setcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_module_init_start():
    _dp_setcomp_3 = __dp_def_fn(
        _dp_bb__dp_setcomp_3_start,
        __dp_decode_literal_bytes(b"<setcomp>"),
        __dp_decode_literal_bytes(b"_dp_setcomp_3"),
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_setcomp_3(it))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_dictcomp_3_0(_dp_tmp_1):
    _dp_tmp_1 = _dp_tmp_1.take()
    return __dp_ret(_dp_tmp_1)


def _dp_bb__dp_dictcomp_3_1(_dp_iter_2, _dp_tmp_1, k, v):
    _dp_iter_2, _dp_tmp_1, k, v = (
        _dp_iter_2.take(),
        _dp_tmp_1.take(),
        k.take(),
        v.take(),
    )
    __dp_setitem(_dp_tmp_1, k, v)
    return __dp_jump(_dp_bb__dp_dictcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_dictcomp_3_2(_dp_iter_2, _dp_tmp_1, _dp_tmp_3):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = (
        _dp_iter_2.take(),
        _dp_tmp_1.take(),
        _dp_tmp_3.take(),
    )
    k = __dp_getitem(_dp_tmp_3, 0)
    v = __dp_getitem(_dp_tmp_3, 1)
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(_dp_bb__dp_dictcomp_3_1, (_dp_iter_2, _dp_tmp_1, k, v))


def _dp_bb__dp_dictcomp_3_3(_dp_iter_2, _dp_tmp_1):
    _dp_iter_2, _dp_tmp_1 = _dp_iter_2.take(), _dp_tmp_1.take()
    _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_3, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb__dp_dictcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_dictcomp_3_2,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_dictcomp_3_start(_dp_iter_2):
    _dp_iter_2 = _dp_iter_2.take()
    _dp_tmp_1 = __dp_dict()
    _dp_iter_2 = __dp_iter(_dp_iter_2)
    return __dp_jump(_dp_bb__dp_dictcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_module_init_start():
    _dp_dictcomp_3 = __dp_def_fn(
        _dp_bb__dp_dictcomp_3_start,
        __dp_decode_literal_bytes(b"<dictcomp>"),
        __dp_decode_literal_bytes(b"_dp_dictcomp_3"),
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_dictcomp_3(it))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# attribute_non_chain

x = f().y

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), f().y)


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_getattr(f(), __dp_decode_literal_bytes(b"y")),
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# fstring_simple

x = f"{a}"

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_format(a))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), __dp_format(a))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_getattr(_dp_templatelib, __dp_decode_literal_bytes(b"Template"))(
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
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

# complex_literal

x = 1j

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), complex(0.0, 1.0))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), complex(0.0, 1.0))
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start

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
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"x"),
        __dp_float_from_literal(__dp_decode_literal_bytes(b"1.234567890123456789")),
    )
    return __dp_ret(None)


__dp_store_global(
    globals(),
    "_dp_module_init",
    __dp_def_fn(
        _dp_bb__dp_module_init_start,
        "_dp_module_init",
        "_dp_module_init",
        (),
        (),
        __dp_globals(),
        __name__,
        None,
        None,
    ),
)
del _dp_bb__dp_module_init_start
