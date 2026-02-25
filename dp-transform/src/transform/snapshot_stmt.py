# import_simple

import a

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"a"),
        __dp_import_(__dp_decode_literal_bytes(b"a"), __spec__),
    )


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"a"),
        __dp_import_(__dp_decode_literal_bytes(b"a"), __spec__),
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

# import_dotted_alias

import a.b as c

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"c"),
        __dp_import_attr(
            __dp_import_(__dp_decode_literal_bytes(b"a.b"), __spec__),
            __dp_decode_literal_bytes(b"b"),
        ),
    )


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"c"),
        __dp_import_attr(
            __dp_import_(__dp_decode_literal_bytes(b"a.b"), __spec__),
            __dp_decode_literal_bytes(b"b"),
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

# import_from_alias

from pkg.mod import name as alias

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_import_1 = __dp_import_(
        __dp_decode_literal_bytes(b"pkg.mod"),
        __spec__,
        __dp_list((__dp_decode_literal_bytes(b"name"),)),
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"alias"),
        __dp_import_attr(_dp_import_1, __dp_decode_literal_bytes(b"name")),
    )


# -- bb --
def _dp_bb__dp_module_init_start():
    _dp_import_1 = __dp_import_(
        __dp_decode_literal_bytes(b"pkg.mod"),
        __spec__,
        __dp_list((__dp_decode_literal_bytes(b"name"),)),
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"alias"),
        __dp_import_attr(_dp_import_1, __dp_decode_literal_bytes(b"name")),
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

# decorator_function


@dec
def f():
    pass


# ==


# -- pre-bb --
def _dp_module_init():

    @dec
    def f():
        pass


# -- bb --
def _dp_bb_f_start():
    return __dp_ret(None)


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"f"),
        dec(
            __dp_def_fn(
                _dp_bb_f_start,
                __dp_decode_literal_bytes(b"f"),
                __dp_decode_literal_bytes(b"f"),
                (),
                (),
                __dp_globals(),
                __name__,
                __dp_NONE,
                __dp_NONE,
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

# assign_attr

obj.x = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), 1)


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), 1)
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

# assign_subscript

obj[i] = v

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setitem(obj, i, v)


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_setitem(obj, i, v)
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

# assign_tuple_unpack

a, b = it

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = __dp_unpack(it, (True, True))
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"a"), __dp_getitem(_dp_tmp_1, 0)
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"b"), __dp_getitem(_dp_tmp_1, 1)
    )
    del _dp_tmp_1


# -- bb --
def _dp_bb__dp_module_init_start():
    _dp_tmp_1 = __dp_unpack(it, (True, True))
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"a"),
        __dp_getitem(
            __dp_load_deleted_name(__dp_decode_literal_bytes(b"_dp_tmp_1"), _dp_tmp_1),
            0,
        ),
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"b"),
        __dp_getitem(
            __dp_load_deleted_name(__dp_decode_literal_bytes(b"_dp_tmp_1"), _dp_tmp_1),
            1,
        ),
    )
    _dp_tmp_1 = __dp_DELETED
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

# assign_star_unpack

a, *b = it

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = __dp_unpack(it, (True, False))
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"a"), __dp_getitem(_dp_tmp_1, 0)
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"b"),
        __dp_list(__dp_getitem(_dp_tmp_1, 1)),
    )
    del _dp_tmp_1


# -- bb --
def _dp_bb__dp_module_init_start():
    _dp_tmp_1 = __dp_unpack(it, (True, False))
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"a"),
        __dp_getitem(
            __dp_load_deleted_name(__dp_decode_literal_bytes(b"_dp_tmp_1"), _dp_tmp_1),
            0,
        ),
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"b"),
        __dp_list(
            __dp_getitem(
                __dp_load_deleted_name(
                    __dp_decode_literal_bytes(b"_dp_tmp_1"), _dp_tmp_1
                ),
                1,
            )
        ),
    )
    _dp_tmp_1 = __dp_DELETED
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

# assign_multi_targets

a = b = f()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = f()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"a"), _dp_tmp_1)
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"b"), _dp_tmp_1)


# -- bb --
def _dp_bb__dp_module_init_start():
    _dp_tmp_1 = f()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"a"), _dp_tmp_1)
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"b"), _dp_tmp_1)
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

# ann_assign_simple

x: int = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), 1)

    def __annotate__(_dp_format, _dp=__dp__):
        if _dp.eq(_dp_format, 4):
            return _dp.dict(
                ((__dp_decode_literal_bytes(b"x"), __dp_decode_literal_bytes(b"int")),)
            )
        if _dp.gt(_dp_format, 2):
            raise _dp.builtins.NotImplementedError
        return _dp.dict(((__dp_decode_literal_bytes(b"x"), int),))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), 1)
    __annotate__ = __dp_exec_function_def_source(
        __dp_decode_literal_bytes(
            b'def __annotate__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_decode_literal_bytes=__dp_decode_literal_bytes):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(((__dp_decode_literal_bytes(b"x"), __dp_decode_literal_bytes(b"int")),))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(((__dp_decode_literal_bytes(b"x"), int),))'
        ),
        __dp_globals(),
        (),
        __dp_decode_literal_bytes(b"__annotate__"),
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"__annotate__"),
        __dp_update_fn(
            __annotate__,
            __dp_decode_literal_bytes(b"__annotate__"),
            __dp_decode_literal_bytes(b"__annotate__"),
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

# ann_assign_attr

obj.x: int = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), 1)


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), 1)
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

# aug_assign_attr

obj.x += 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), __dp_iadd(obj.x, 1))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_setattr(
        obj,
        __dp_decode_literal_bytes(b"x"),
        __dp_iadd(__dp_getattr(obj, __dp_decode_literal_bytes(b"x")), 1),
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

# delete_mixed

del obj.x, obj[i], x

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_delattr(obj, __dp_decode_literal_bytes(b"x"))
    __dp_delitem(obj, i)
    __dp_delitem(globals(), __dp_decode_literal_bytes(b"x"))


# -- bb --
def _dp_bb__dp_module_init_start():
    __dp_delattr(obj, __dp_decode_literal_bytes(b"x"))
    __dp_delitem(obj, i)
    __dp_delitem(globals(), __dp_decode_literal_bytes(b"x"))
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

# assert_no_msg

assert cond

# ==


# -- pre-bb --
def _dp_module_init():
    if __debug__:
        if __dp_not_(cond):
            raise __dp_AssertionError


# -- bb --
def _dp_bb__dp_module_init_0():
    return __dp_raise_(__dp_AssertionError)


def _dp_bb__dp_module_init_1():
    return __dp_brif(
        __dp_not_(cond), _dp_bb__dp_module_init_0, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_start():
    return __dp_brif(
        __debug__, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_2():
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

# assert_with_msg

assert cond, "oops"

# ==


# -- pre-bb --
def _dp_module_init():
    if __debug__:
        if __dp_not_(cond):
            raise __dp_AssertionError(__dp_decode_literal_bytes(b"oops"))


# -- bb --
def _dp_bb__dp_module_init_0():
    return __dp_raise_(__dp_AssertionError(__dp_decode_literal_bytes(b"oops")))


def _dp_bb__dp_module_init_1():
    return __dp_brif(
        __dp_not_(cond), _dp_bb__dp_module_init_0, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_start():
    return __dp_brif(
        __debug__, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_2():
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

# raise_from

raise E from cause

# ==


# -- pre-bb --
def _dp_module_init():
    raise __dp_raise_from(E, cause)


# -- bb --
def _dp_bb__dp_module_init_start():
    return __dp_raise_(__dp_raise_from(E, cause))


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

# try_except_typed

try:
    f()
except E as e:
    g(e)
except:
    h()

# ==


# -- pre-bb --
def _dp_module_init():
    try:
        f()
    except:
        if __dp_exception_matches(__dp_current_exception(), E):
            __dp_store_global(
                globals(), __dp_decode_literal_bytes(b"e"), __dp_current_exception()
            )
            try:
                g(__dp_load_global(globals(), __dp_decode_literal_bytes(b"e")))
            finally:
                __dp_delitem_quietly(globals(), __dp_decode_literal_bytes(b"e"))
        else:
            h()


# -- bb --
def _dp_bb__dp_module_init_0(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    f()
    return __dp_jump(
        _dp_bb__dp_module_init_0, (locals().get("_dp_try_exc_4", __dp_DELETED),)
    )


def _dp_bb__dp_module_init_2():
    _dp_try_exc_4 = __dp_DELETED
    return __dp_ret(None)


def _dp_bb__dp_module_init_3(_dp_try_exc_5):
    _dp_try_exc_5 = _dp_try_exc_5.take()
    return __dp_raise_(_dp_try_exc_5)


def _dp_bb__dp_module_init_4(_dp_try_exc_5):
    _dp_try_exc_5 = _dp_try_exc_5.take()
    __dp_delitem_quietly(globals(), __dp_decode_literal_bytes(b"e"))
    return __dp_brif(
        __dp_is_not(_dp_try_exc_5, __dp_NONE),
        _dp_bb__dp_module_init_3,
        (locals().get("_dp_try_exc_5", __dp_DELETED),),
        _dp_bb__dp_module_init_2,
        (),
    )


def _dp_bb__dp_module_init_5(_dp_try_exc_11):
    _dp_try_exc_11 = _dp_try_exc_11.take()
    _dp_try_exc_5 = __dp_NONE
    return __dp_jump(
        _dp_bb__dp_module_init_4, (locals().get("_dp_try_exc_5", __dp_DELETED),)
    )


def _dp_bb__dp_module_init_6(_dp_try_exc_5, _dp_try_exc_11):
    _dp_try_exc_5, _dp_try_exc_11 = _dp_try_exc_5.take(), _dp_try_exc_11.take()
    g(__dp_load_global(globals(), __dp_decode_literal_bytes(b"e")))
    return __dp_jump(
        _dp_bb__dp_module_init_5, (locals().get("_dp_try_exc_11", __dp_DELETED),)
    )


def _dp_bb__dp_module_init_7(_dp_try_exc_5, _dp_try_exc_11):
    _dp_try_exc_5, _dp_try_exc_11 = _dp_try_exc_5.take(), _dp_try_exc_11.take()
    return __dp_raise_(_dp_try_exc_11)


def _dp_bb__dp_module_init_8(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"e"), _dp_try_exc_4)
    return __dp_jump(
        _dp_bb__dp_module_init_6,
        (
            locals().get("_dp_try_exc_5", __dp_DELETED),
            locals().get("_dp_try_exc_11", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_9():
    h()
    return __dp_jump(_dp_bb__dp_module_init_2, ())


def _dp_bb__dp_module_init_10(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_4, E),
        _dp_bb__dp_module_init_8,
        (locals().get("_dp_try_exc_4", __dp_DELETED),),
        _dp_bb__dp_module_init_9,
        (),
    )


def _dp_bb__dp_module_init_start():
    return __dp_jump(
        _dp_bb__dp_module_init_1, (locals().get("_dp_try_exc_4", __dp_DELETED),)
    )


__dp_setattr(_dp_bb__dp_module_init_0, "_dp_exc_target", _dp_bb__dp_module_init_10)
__dp_setattr(_dp_bb__dp_module_init_0, "_dp_exc_name", "_dp_try_exc_4")
__dp_setattr(_dp_bb__dp_module_init_1, "_dp_exc_target", _dp_bb__dp_module_init_10)
__dp_setattr(_dp_bb__dp_module_init_1, "_dp_exc_name", "_dp_try_exc_4")
__dp_setattr(_dp_bb__dp_module_init_5, "_dp_exc_target", _dp_bb__dp_module_init_7)
__dp_setattr(_dp_bb__dp_module_init_5, "_dp_exc_name", "_dp_try_exc_11")
__dp_setattr(_dp_bb__dp_module_init_6, "_dp_exc_target", _dp_bb__dp_module_init_7)
__dp_setattr(_dp_bb__dp_module_init_6, "_dp_exc_name", "_dp_try_exc_11")
__dp_setattr(_dp_bb__dp_module_init_7, "_dp_exc_target", _dp_bb__dp_module_init_4)
__dp_setattr(_dp_bb__dp_module_init_7, "_dp_exc_name", "_dp_try_exc_5")
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

# for_else

for x in it:
    body()
else:
    done()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_iter_1 = __dp_iter(it)
    _dp_completed_3 = False
    while __dp_not_(_dp_completed_3):
        _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
        if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
            _dp_completed_3 = True
        else:
            __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), _dp_tmp_2)
            _dp_tmp_2 = None
            body()
    else:
        done()


# -- bb --
def _dp_bb__dp_module_init_0():
    done()
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(x, _dp_iter_2):
    x, _dp_iter_2 = x.take(), _dp_iter_2.take()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"x"), x)
    body()
    return __dp_jump(_dp_bb__dp_module_init_3, (_dp_iter_2,))


def _dp_bb__dp_module_init_2(_dp_tmp_3, _dp_iter_2):
    _dp_tmp_3, _dp_iter_2 = _dp_tmp_3.take(), _dp_iter_2.take()
    x = _dp_tmp_3
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(_dp_bb__dp_module_init_1, (x, _dp_iter_2))


def _dp_bb__dp_module_init_3(_dp_iter_2):
    _dp_iter_2 = _dp_iter_2.take()
    _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_3, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb__dp_module_init_0,
        (),
        _dp_bb__dp_module_init_2,
        (_dp_tmp_3, _dp_iter_2),
    )


def _dp_bb__dp_module_init_start():
    _dp_iter_2 = __dp_iter(it)
    return __dp_jump(_dp_bb__dp_module_init_3, (_dp_iter_2,))


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

# while_else

while cond:
    body()
else:
    done()

# ==


# -- pre-bb --
def _dp_module_init():
    while cond:
        body()
    else:
        done()


# -- bb --
def _dp_bb__dp_module_init_0():
    done()
    return __dp_ret(None)


def _dp_bb__dp_module_init_1():
    body()
    return __dp_jump(_dp_bb__dp_module_init_start, ())


def _dp_bb__dp_module_init_start():
    return __dp_brif(cond, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_0, ())


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

# with_as

with cm as x:
    body()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_contextmanager_enter(cm)
    )
    _dp_with_ok_2 = True
    try:
        body()
    except:
        if __dp_exception_matches(__dp_current_exception(), BaseException):
            _dp_with_ok_2 = False
            __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
        else:
            raise
    if _dp_with_ok_2:
        __dp_contextmanager_exit(_dp_with_exit_1, None)
    _dp_with_exit_1 = None


# -- bb --
def _dp_bb__dp_module_init_0():
    _dp_with_exit_1 = __dp_NONE
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(_dp_with_exit_1):
    _dp_with_exit_1 = _dp_with_exit_1.take()
    __dp_contextmanager_exit(_dp_with_exit_1, __dp_NONE)
    return __dp_jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_with_exit_1, _dp_with_ok_2):
    _dp_with_exit_1, _dp_with_ok_2 = _dp_with_exit_1.take(), _dp_with_ok_2.take()
    return __dp_brif(
        _dp_with_ok_2,
        _dp_bb__dp_module_init_1,
        (_dp_with_exit_1,),
        _dp_bb__dp_module_init_0,
        (),
    )


def _dp_bb__dp_module_init_3(_dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_7):
    _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_7 = (
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_7.take(),
    )
    return __dp_jump(_dp_bb__dp_module_init_2, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_4(_dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_7):
    _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_7 = (
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_7.take(),
    )
    body()
    return __dp_jump(
        _dp_bb__dp_module_init_3,
        (_dp_with_exit_1, _dp_with_ok_2, locals().get("_dp_try_exc_7", __dp_DELETED)),
    )


def _dp_bb__dp_module_init_5(_dp_with_exit_1, _dp_with_ok_2):
    _dp_with_exit_1, _dp_with_ok_2 = _dp_with_exit_1.take(), _dp_with_ok_2.take()
    _dp_try_exc_7 = __dp_DELETED
    return __dp_jump(_dp_bb__dp_module_init_2, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_6(_dp_with_exit_1, _dp_try_exc_7):
    _dp_with_exit_1, _dp_try_exc_7 = _dp_with_exit_1.take(), _dp_try_exc_7.take()
    _dp_with_ok_2 = __dp_FALSE
    __dp_contextmanager_exit(
        _dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_7)
    )
    return __dp_jump(_dp_bb__dp_module_init_5, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_7(_dp_try_exc_7):
    _dp_try_exc_7 = _dp_try_exc_7.take()
    return __dp_raise_(_dp_try_exc_7)


def _dp_bb__dp_module_init_8(_dp_with_exit_1, _dp_try_exc_7):
    _dp_with_exit_1, _dp_try_exc_7 = _dp_with_exit_1.take(), _dp_try_exc_7.take()
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_7, BaseException),
        _dp_bb__dp_module_init_6,
        (_dp_with_exit_1, locals().get("_dp_try_exc_7", __dp_DELETED)),
        _dp_bb__dp_module_init_7,
        (locals().get("_dp_try_exc_7", __dp_DELETED),),
    )


def _dp_bb__dp_module_init_start():
    _dp_with_exit_1 = __dp_contextmanager_get_exit(cm)
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_contextmanager_enter(cm)
    )
    _dp_with_ok_2 = __dp_TRUE
    return __dp_jump(
        _dp_bb__dp_module_init_4,
        (_dp_with_exit_1, _dp_with_ok_2, locals().get("_dp_try_exc_7", __dp_DELETED)),
    )


__dp_setattr(_dp_bb__dp_module_init_3, "_dp_exc_target", _dp_bb__dp_module_init_8)
__dp_setattr(_dp_bb__dp_module_init_3, "_dp_exc_name", "_dp_try_exc_7")
__dp_setattr(_dp_bb__dp_module_init_4, "_dp_exc_target", _dp_bb__dp_module_init_8)
__dp_setattr(_dp_bb__dp_module_init_4, "_dp_exc_name", "_dp_try_exc_7")
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

# function_local_ann_assign


def inner():
    value: int = 1
    return value


# ==


# -- pre-bb --
def _dp_module_init():

    def inner():
        value = 1
        return value


# -- bb --
def _dp_bb_inner_start():
    value = 1
    return __dp_ret(value)


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"inner"),
        __dp_def_fn(
            _dp_bb_inner_start,
            __dp_decode_literal_bytes(b"inner"),
            __dp_decode_literal_bytes(b"inner"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# comprehension_global

xs = [x for x in it]
ys = {x for x in it}
zs = {k: v for k, v in items}

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
                x = _dp_tmp_5
                _dp_tmp_5 = None
                _dp_tmp_1.append(x)
        return _dp_tmp_1

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"xs"), _dp_listcomp_3(it))

    def _dp_setcomp_9(_dp_iter_8):
        _dp_tmp_7 = set()
        _dp_iter_10 = __dp_iter(_dp_iter_8)
        while True:
            _dp_tmp_11 = __dp_next_or_sentinel(_dp_iter_10)
            if __dp_is_(_dp_tmp_11, __dp__.ITER_COMPLETE):
                break
            else:
                x = _dp_tmp_11
                _dp_tmp_11 = None
                _dp_tmp_7.add(x)
        return _dp_tmp_7

    __dp_store_global(globals(), __dp_decode_literal_bytes(b"ys"), _dp_setcomp_9(it))

    def _dp_dictcomp_15(_dp_iter_14):
        _dp_tmp_13 = __dp_dict()
        _dp_iter_16 = __dp_iter(_dp_iter_14)
        while True:
            _dp_tmp_17 = __dp_next_or_sentinel(_dp_iter_16)
            if __dp_is_(_dp_tmp_17, __dp__.ITER_COMPLETE):
                break
            else:
                _dp_tmp_19 = _dp_tmp_17
                k = __dp_getitem(_dp_tmp_19, 0)
                v = __dp_getitem(_dp_tmp_19, 1)
                del _dp_tmp_19
                _dp_tmp_17 = None
                __dp_setitem(_dp_tmp_13, k, v)
        return _dp_tmp_13

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"zs"), _dp_dictcomp_15(items)
    )


# -- bb --
def _dp_bb__dp_listcomp_3_0(_dp_tmp_1):
    _dp_tmp_1 = _dp_tmp_1.take()
    return __dp_ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_iter_2, _dp_tmp_1, x):
    _dp_iter_2, _dp_tmp_1, x = _dp_iter_2.take(), _dp_tmp_1.take(), x.take()
    __dp_getattr(_dp_tmp_1, __dp_decode_literal_bytes(b"append"))(x)
    return __dp_jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_iter_2, _dp_tmp_1, _dp_tmp_3):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = (
        _dp_iter_2.take(),
        _dp_tmp_1.take(),
        _dp_tmp_3.take(),
    )
    x = _dp_tmp_3
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(_dp_bb__dp_listcomp_3_1, (_dp_iter_2, _dp_tmp_1, x))


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


def _dp_bb__dp_setcomp_6_0(_dp_tmp_4):
    _dp_tmp_4 = _dp_tmp_4.take()
    return __dp_ret(_dp_tmp_4)


def _dp_bb__dp_setcomp_6_1(_dp_tmp_4, x, _dp_iter_10):
    _dp_tmp_4, x, _dp_iter_10 = _dp_tmp_4.take(), x.take(), _dp_iter_10.take()
    __dp_getattr(_dp_tmp_4, __dp_decode_literal_bytes(b"add"))(x)
    return __dp_jump(_dp_bb__dp_setcomp_6_3, (_dp_tmp_4, _dp_iter_10))


def _dp_bb__dp_setcomp_6_2(_dp_tmp_4, _dp_tmp_11, _dp_iter_10):
    _dp_tmp_4, _dp_tmp_11, _dp_iter_10 = (
        _dp_tmp_4.take(),
        _dp_tmp_11.take(),
        _dp_iter_10.take(),
    )
    x = _dp_tmp_11
    _dp_tmp_11 = __dp_NONE
    return __dp_jump(_dp_bb__dp_setcomp_6_1, (_dp_tmp_4, x, _dp_iter_10))


def _dp_bb__dp_setcomp_6_3(_dp_tmp_4, _dp_iter_10):
    _dp_tmp_4, _dp_iter_10 = _dp_tmp_4.take(), _dp_iter_10.take()
    _dp_tmp_11 = __dp_next_or_sentinel(_dp_iter_10)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_11,
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE")),
        ),
        _dp_bb__dp_setcomp_6_0,
        (_dp_tmp_4,),
        _dp_bb__dp_setcomp_6_2,
        (_dp_tmp_4, _dp_tmp_11, _dp_iter_10),
    )


def _dp_bb__dp_setcomp_6_start(_dp_iter_5):
    _dp_iter_5 = _dp_iter_5.take()
    _dp_tmp_4 = set()
    _dp_iter_10 = __dp_iter(_dp_iter_5)
    return __dp_jump(_dp_bb__dp_setcomp_6_3, (_dp_tmp_4, _dp_iter_10))


def _dp_bb__dp_dictcomp_9_0(_dp_tmp_7):
    _dp_tmp_7 = _dp_tmp_7.take()
    return __dp_ret(_dp_tmp_7)


def _dp_bb__dp_dictcomp_9_1(_dp_tmp_7, k, v, _dp_iter_18):
    _dp_tmp_7, k, v, _dp_iter_18 = (
        _dp_tmp_7.take(),
        k.take(),
        v.take(),
        _dp_iter_18.take(),
    )
    __dp_setitem(_dp_tmp_7, k, v)
    return __dp_jump(_dp_bb__dp_dictcomp_9_3, (_dp_tmp_7, _dp_iter_18))


def _dp_bb__dp_dictcomp_9_2(_dp_tmp_7, _dp_tmp_19, _dp_iter_18):
    _dp_tmp_7, _dp_tmp_19, _dp_iter_18 = (
        _dp_tmp_7.take(),
        _dp_tmp_19.take(),
        _dp_iter_18.take(),
    )
    k = __dp_getitem(_dp_tmp_19, 0)
    v = __dp_getitem(_dp_tmp_19, 1)
    _dp_tmp_19 = __dp_NONE
    return __dp_jump(_dp_bb__dp_dictcomp_9_1, (_dp_tmp_7, k, v, _dp_iter_18))


def _dp_bb__dp_dictcomp_9_3(_dp_tmp_7, _dp_iter_18):
    _dp_tmp_7, _dp_iter_18 = _dp_tmp_7.take(), _dp_iter_18.take()
    _dp_tmp_19 = __dp_next_or_sentinel(_dp_iter_18)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_19,
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE")),
        ),
        _dp_bb__dp_dictcomp_9_0,
        (_dp_tmp_7,),
        _dp_bb__dp_dictcomp_9_2,
        (_dp_tmp_7, _dp_tmp_19, _dp_iter_18),
    )


def _dp_bb__dp_dictcomp_9_start(_dp_iter_8):
    _dp_iter_8 = _dp_iter_8.take()
    _dp_tmp_7 = __dp_dict()
    _dp_iter_18 = __dp_iter(_dp_iter_8)
    return __dp_jump(_dp_bb__dp_dictcomp_9_3, (_dp_tmp_7, _dp_iter_18))


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
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"xs"), _dp_listcomp_3(it))
    _dp_setcomp_6 = __dp_def_fn(
        _dp_bb__dp_setcomp_6_start,
        __dp_decode_literal_bytes(b"<setcomp>"),
        __dp_decode_literal_bytes(b"_dp_setcomp_6"),
        ("_dp_iter_5",),
        (("_dp_iter_5", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"ys"), _dp_setcomp_6(it))
    _dp_dictcomp_9 = __dp_def_fn(
        _dp_bb__dp_dictcomp_9_start,
        __dp_decode_literal_bytes(b"<dictcomp>"),
        __dp_decode_literal_bytes(b"_dp_dictcomp_9"),
        ("_dp_iter_8",),
        (("_dp_iter_8", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"zs"), _dp_dictcomp_9(items)
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

# comprehension_in_function


def f():
    return [x for x in it if x > 0]


# ==


# -- pre-bb --
def _dp_module_init():

    def f():

        def _dp_listcomp_3(_dp_iter_2):
            _dp_tmp_1 = __dp_list(())
            _dp_iter_4 = __dp_iter(_dp_iter_2)
            while True:
                _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
                if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
                    break
                else:
                    x = _dp_tmp_5
                    _dp_tmp_5 = None
                    if __dp_gt(x, 0):
                        _dp_tmp_1.append(x)
            return _dp_tmp_1

        return _dp_listcomp_3(it)


# -- bb --
def _dp_bb__dp_listcomp_3_0(_dp_tmp_1):
    _dp_tmp_1 = _dp_tmp_1.take()
    return __dp_ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_iter_2, _dp_tmp_1, x):
    _dp_iter_2, _dp_tmp_1, x = _dp_iter_2.take(), _dp_tmp_1.take(), x.take()
    __dp_getattr(_dp_tmp_1, __dp_decode_literal_bytes(b"append"))(x)
    return __dp_jump(_dp_bb__dp_listcomp_3_4, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_iter_2, _dp_tmp_1, x):
    _dp_iter_2, _dp_tmp_1, x = _dp_iter_2.take(), _dp_tmp_1.take(), x.take()
    return __dp_brif(
        __dp_gt(x, 0),
        _dp_bb__dp_listcomp_3_1,
        (_dp_iter_2, _dp_tmp_1, x),
        _dp_bb__dp_listcomp_3_4,
        (_dp_iter_2, _dp_tmp_1),
    )


def _dp_bb__dp_listcomp_3_3(_dp_iter_2, _dp_tmp_1, _dp_tmp_3):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = (
        _dp_iter_2.take(),
        _dp_tmp_1.take(),
        _dp_tmp_3.take(),
    )
    x = _dp_tmp_3
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(_dp_bb__dp_listcomp_3_2, (_dp_iter_2, _dp_tmp_1, x))


def _dp_bb__dp_listcomp_3_4(_dp_iter_2, _dp_tmp_1):
    _dp_iter_2, _dp_tmp_1 = _dp_iter_2.take(), _dp_tmp_1.take()
    _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_3, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb__dp_listcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_listcomp_3_3,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_listcomp_3_start(_dp_iter_2):
    _dp_iter_2 = _dp_iter_2.take()
    _dp_tmp_1 = __dp_list(())
    _dp_iter_2 = __dp_iter(_dp_iter_2)
    return __dp_jump(_dp_bb__dp_listcomp_3_4, (_dp_iter_2, _dp_tmp_1))


def _dp_bb_f_start():
    _dp_listcomp_3 = __dp_def_fn(
        _dp_bb__dp_listcomp_3_start,
        __dp_decode_literal_bytes(b"<listcomp>"),
        __dp_decode_literal_bytes(b"f.<locals>._dp_listcomp_3"),
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    return __dp_ret(_dp_listcomp_3(it))


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"f"),
        __dp_def_fn(
            _dp_bb_f_start,
            __dp_decode_literal_bytes(b"f"),
            __dp_decode_literal_bytes(b"f"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# comprehension_in_class_body


class C:
    xs = [x for x in it]


# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_C(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"C"),
        )

        def _dp_listcomp_3(_dp_iter_2):
            _dp_tmp_1 = __dp_list(())
            _dp_iter_4 = __dp_iter(_dp_iter_2)
            while True:
                _dp_tmp_5 = __dp_next_or_sentinel(_dp_iter_4)
                if __dp_is_(_dp_tmp_5, __dp__.ITER_COMPLETE):
                    break
                else:
                    x = _dp_tmp_5
                    _dp_tmp_5 = None
                    _dp_tmp_1.append(x)
            return _dp_tmp_1

        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"xs"),
            _dp_listcomp_3(
                __dp_class_lookup_global(
                    _dp_class_ns, __dp_decode_literal_bytes(b"it"), globals()
                )
            ),
        )

    def _dp_define_class_C(_dp_class_ns_fn):
        return __dp_create_class(
            __dp_decode_literal_bytes(b"C"), _dp_class_ns_fn, (), None, False, 3, ()
        )

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"C"), _dp_define_class_C(_dp_class_ns_C)
    )


# -- bb --
def _dp_bb__dp_module_init_start():

    def _dp_bb__dp_listcomp_3_0(_dp_tmp_1):
        _dp_tmp_1 = _dp_tmp_1.take()
        return __dp_ret(_dp_tmp_1)

    def _dp_bb__dp_listcomp_3_1(_dp_iter_2, _dp_tmp_1, x):
        _dp_iter_2, _dp_tmp_1, x = _dp_iter_2.take(), _dp_tmp_1.take(), x.take()
        __dp_getattr(_dp_tmp_1, __dp_decode_literal_bytes(b"append"))(x)
        return __dp_jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))

    def _dp_bb__dp_listcomp_3_2(_dp_iter_2, _dp_tmp_1, _dp_tmp_3):
        _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = (
            _dp_iter_2.take(),
            _dp_tmp_1.take(),
            _dp_tmp_3.take(),
        )
        x = _dp_tmp_3
        _dp_tmp_3 = __dp_NONE
        return __dp_jump(_dp_bb__dp_listcomp_3_1, (_dp_iter_2, _dp_tmp_1, x))

    def _dp_bb__dp_listcomp_3_3(_dp_iter_2, _dp_tmp_1):
        _dp_iter_2, _dp_tmp_1 = _dp_iter_2.take(), _dp_tmp_1.take()
        _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
        return __dp_brif(
            __dp_is_(
                _dp_tmp_3,
                __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE")),
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

    def _dp_bb__dp_class_ns_C_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"C"),
        )
        _dp_listcomp_3 = __dp_def_fn(
            _dp_bb__dp_listcomp_3_start,
            __dp_decode_literal_bytes(b"<listcomp>"),
            __dp_decode_literal_bytes(b"C._dp_listcomp_3"),
            ("_dp_iter_2",),
            (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
        )
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"xs"),
            _dp_listcomp_3(
                __dp_class_lookup_global(
                    _dp_class_ns, __dp_decode_literal_bytes(b"it"), globals()
                )
            ),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_C_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"C"),
                _dp_class_ns_fn,
                (),
                __dp_NONE,
                __dp_FALSE,
                3,
                (),
            )
        )

    _dp_class_ns_C = __dp_def_fn(
        _dp_bb__dp_class_ns_C_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_C"),
        __dp_decode_literal_bytes(b"_dp_class_ns_C"),
        ("_dp_class_ns", "_dp_classcell_arg"),
        (
            ("_dp_class_ns", None, __dp__.NO_DEFAULT),
            ("_dp_classcell_arg", None, __dp__.NO_DEFAULT),
        ),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    _dp_define_class_C = __dp_def_fn(
        _dp_bb__dp_define_class_C_start,
        __dp_decode_literal_bytes(b"_dp_define_class_C"),
        __dp_decode_literal_bytes(b"_dp_define_class_C"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"C"), _dp_define_class_C(_dp_class_ns_C)
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

# with_multi

with a as x, b as y:
    body()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_with_exit_3 = __dp_contextmanager_get_exit(a)
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_contextmanager_enter(a)
    )
    _dp_with_ok_4 = True
    try:
        _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
        __dp_store_global(
            globals(), __dp_decode_literal_bytes(b"y"), __dp_contextmanager_enter(b)
        )
        _dp_with_ok_2 = True
        try:
            body()
        except:
            if __dp_exception_matches(__dp_current_exception(), BaseException):
                _dp_with_ok_2 = False
                __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
            else:
                raise
        if _dp_with_ok_2:
            __dp_contextmanager_exit(_dp_with_exit_1, None)
        _dp_with_exit_1 = None
    except:
        if __dp_exception_matches(__dp_current_exception(), BaseException):
            _dp_with_ok_4 = False
            __dp_contextmanager_exit(_dp_with_exit_3, __dp_exc_info())
        else:
            raise
    if _dp_with_ok_4:
        __dp_contextmanager_exit(_dp_with_exit_3, None)
    _dp_with_exit_3 = None


# -- bb --
def _dp_bb__dp_module_init_0():
    _dp_with_exit_3 = __dp_NONE
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(_dp_with_exit_3):
    _dp_with_exit_3 = _dp_with_exit_3.take()
    __dp_contextmanager_exit(_dp_with_exit_3, __dp_NONE)
    return __dp_jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_with_exit_3, _dp_with_ok_4):
    _dp_with_exit_3, _dp_with_ok_4 = _dp_with_exit_3.take(), _dp_with_ok_4.take()
    return __dp_brif(
        _dp_with_ok_4,
        _dp_bb__dp_module_init_1,
        (_dp_with_exit_3,),
        _dp_bb__dp_module_init_0,
        (),
    )


def _dp_bb__dp_module_init_3(_dp_with_exit_3, _dp_with_ok_4, _dp_try_exc_17):
    _dp_with_exit_3, _dp_with_ok_4, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_try_exc_17.take(),
    )
    return __dp_jump(_dp_bb__dp_module_init_2, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_4(_dp_with_exit_3, _dp_with_ok_4, _dp_try_exc_17):
    _dp_with_exit_3, _dp_with_ok_4, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_try_exc_17.take(),
    )
    _dp_with_exit_1 = __dp_NONE
    return __dp_jump(
        _dp_bb__dp_module_init_3,
        (_dp_with_exit_3, _dp_with_ok_4, locals().get("_dp_try_exc_17", __dp_DELETED)),
    )


def _dp_bb__dp_module_init_5(
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_try_exc_17
):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_try_exc_17.take(),
    )
    __dp_contextmanager_exit(_dp_with_exit_1, __dp_NONE)
    return __dp_jump(
        _dp_bb__dp_module_init_4,
        (_dp_with_exit_3, _dp_with_ok_4, locals().get("_dp_try_exc_17", __dp_DELETED)),
    )


def _dp_bb__dp_module_init_6(
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_17
):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_17.take(),
    )
    return __dp_brif(
        _dp_with_ok_2,
        _dp_bb__dp_module_init_5,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
        _dp_bb__dp_module_init_4,
        (_dp_with_exit_3, _dp_with_ok_4, locals().get("_dp_try_exc_17", __dp_DELETED)),
    )


def _dp_bb__dp_module_init_7(
    _dp_with_exit_3,
    _dp_with_ok_4,
    _dp_with_exit_1,
    _dp_with_ok_2,
    _dp_try_exc_11,
    _dp_try_exc_17,
):
    (
        _dp_with_exit_3,
        _dp_with_ok_4,
        _dp_with_exit_1,
        _dp_with_ok_2,
        _dp_try_exc_11,
        _dp_try_exc_17,
    ) = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_11.take(),
        _dp_try_exc_17.take(),
    )
    return __dp_jump(
        _dp_bb__dp_module_init_6,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_8(
    _dp_with_exit_3,
    _dp_with_ok_4,
    _dp_with_exit_1,
    _dp_with_ok_2,
    _dp_try_exc_11,
    _dp_try_exc_17,
):
    (
        _dp_with_exit_3,
        _dp_with_ok_4,
        _dp_with_exit_1,
        _dp_with_ok_2,
        _dp_try_exc_11,
        _dp_try_exc_17,
    ) = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_11.take(),
        _dp_try_exc_17.take(),
    )
    body()
    return __dp_jump(
        _dp_bb__dp_module_init_7,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_11", __dp_DELETED),
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_9(
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_17
):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_17.take(),
    )
    _dp_try_exc_11 = __dp_DELETED
    return __dp_jump(
        _dp_bb__dp_module_init_6,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_10(
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_try_exc_11, _dp_try_exc_17
):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_try_exc_11, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_try_exc_11.take(),
        _dp_try_exc_17.take(),
    )
    _dp_with_ok_2 = __dp_FALSE
    __dp_contextmanager_exit(
        _dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_11)
    )
    return __dp_jump(
        _dp_bb__dp_module_init_9,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_11(_dp_with_exit_3, _dp_try_exc_11, _dp_try_exc_17):
    _dp_with_exit_3, _dp_try_exc_11, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_try_exc_11.take(),
        _dp_try_exc_17.take(),
    )
    return __dp_raise_(_dp_try_exc_11)


def _dp_bb__dp_module_init_12(
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_try_exc_17, _dp_try_exc_11
):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_try_exc_17, _dp_try_exc_11 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_with_exit_1.take(),
        _dp_try_exc_17.take(),
        _dp_try_exc_11.take(),
    )
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_11, BaseException),
        _dp_bb__dp_module_init_10,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            locals().get("_dp_try_exc_11", __dp_DELETED),
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
        _dp_bb__dp_module_init_11,
        (
            _dp_with_exit_3,
            locals().get("_dp_try_exc_11", __dp_DELETED),
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_13(_dp_with_exit_3, _dp_with_ok_4, _dp_try_exc_17):
    _dp_with_exit_3, _dp_with_ok_4, _dp_try_exc_17 = (
        _dp_with_exit_3.take(),
        _dp_with_ok_4.take(),
        _dp_try_exc_17.take(),
    )
    _dp_with_exit_1 = __dp_contextmanager_get_exit(b)
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"y"), __dp_contextmanager_enter(b)
    )
    _dp_with_ok_2 = __dp_TRUE
    return __dp_jump(
        _dp_bb__dp_module_init_8,
        (
            _dp_with_exit_3,
            _dp_with_ok_4,
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_11", __dp_DELETED),
            locals().get("_dp_try_exc_17", __dp_DELETED),
        ),
    )


def _dp_bb__dp_module_init_14(_dp_with_exit_3, _dp_with_ok_4):
    _dp_with_exit_3, _dp_with_ok_4 = _dp_with_exit_3.take(), _dp_with_ok_4.take()
    _dp_try_exc_17 = __dp_DELETED
    return __dp_jump(_dp_bb__dp_module_init_2, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_15(_dp_with_exit_3, _dp_try_exc_17):
    _dp_with_exit_3, _dp_try_exc_17 = _dp_with_exit_3.take(), _dp_try_exc_17.take()
    _dp_with_ok_4 = __dp_FALSE
    __dp_contextmanager_exit(
        _dp_with_exit_3, __dp_exc_info_from_exception(_dp_try_exc_17)
    )
    return __dp_jump(_dp_bb__dp_module_init_14, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_16(_dp_try_exc_17):
    _dp_try_exc_17 = _dp_try_exc_17.take()
    return __dp_raise_(_dp_try_exc_17)


def _dp_bb__dp_module_init_17(_dp_with_exit_3, _dp_try_exc_17):
    _dp_with_exit_3, _dp_try_exc_17 = _dp_with_exit_3.take(), _dp_try_exc_17.take()
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_17, BaseException),
        _dp_bb__dp_module_init_15,
        (_dp_with_exit_3, locals().get("_dp_try_exc_17", __dp_DELETED)),
        _dp_bb__dp_module_init_16,
        (locals().get("_dp_try_exc_17", __dp_DELETED),),
    )


def _dp_bb__dp_module_init_start():
    _dp_with_exit_3 = __dp_contextmanager_get_exit(a)
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"x"), __dp_contextmanager_enter(a)
    )
    _dp_with_ok_4 = __dp_TRUE
    return __dp_jump(
        _dp_bb__dp_module_init_13,
        (_dp_with_exit_3, _dp_with_ok_4, locals().get("_dp_try_exc_17", __dp_DELETED)),
    )


__dp_setattr(_dp_bb__dp_module_init_3, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_3, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_4, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_4, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_5, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_5, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_6, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_6, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_7, "_dp_exc_target", _dp_bb__dp_module_init_12)
__dp_setattr(_dp_bb__dp_module_init_7, "_dp_exc_name", "_dp_try_exc_11")
__dp_setattr(_dp_bb__dp_module_init_8, "_dp_exc_target", _dp_bb__dp_module_init_12)
__dp_setattr(_dp_bb__dp_module_init_8, "_dp_exc_name", "_dp_try_exc_11")
__dp_setattr(_dp_bb__dp_module_init_9, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_9, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_10, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_10, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_11, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_11, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_12, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_12, "_dp_exc_name", "_dp_try_exc_17")
__dp_setattr(_dp_bb__dp_module_init_13, "_dp_exc_target", _dp_bb__dp_module_init_17)
__dp_setattr(_dp_bb__dp_module_init_13, "_dp_exc_name", "_dp_try_exc_17")
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

# async_for


async def run():
    async for x in ait:
        body()


# ==


# -- pre-bb --
def _dp_module_init():

    async def run():
        _dp_iter_1 = __dp_aiter(ait)
        while True:
            _dp_tmp_2 = await __dp_anext_or_sentinel(_dp_iter_1)
            if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
                break
            else:
                x = _dp_tmp_2
                _dp_tmp_2 = None
                body()


# -- bb --
async def _dp_bb_run_0(_dp_iter_1):
    _dp_iter_1 = _dp_iter_1.take()
    body()
    return __dp_jump(_dp_bb_run_2, (_dp_iter_1,))


async def _dp_bb_run_1(_dp_tmp_2, _dp_iter_1):
    _dp_tmp_2, _dp_iter_1 = _dp_tmp_2.take(), _dp_iter_1.take()
    x = _dp_tmp_2
    _dp_tmp_2 = __dp_NONE
    return __dp_jump(_dp_bb_run_0, (_dp_iter_1,))


async def _dp_bb_run_2(_dp_iter_1):
    _dp_iter_1 = _dp_iter_1.take()
    _dp_tmp_2 = await __dp_anext_or_sentinel(_dp_iter_1)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_2, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb_run_3,
        (),
        _dp_bb_run_1,
        (_dp_tmp_2, _dp_iter_1),
    )


async def _dp_bb_run_start():
    _dp_iter_1 = __dp_aiter(ait)
    return __dp_jump(_dp_bb_run_2, (_dp_iter_1,))


async def _dp_bb_run_3():
    return __dp_ret(None)


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"run"),
        __dp_def_coro(
            _dp_bb_run_start,
            __dp_decode_literal_bytes(b"run"),
            __dp_decode_literal_bytes(b"run"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# async_with


async def run():
    async with cm as x:
        body()


# ==


# -- pre-bb --
def _dp_module_init():

    async def run():
        _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
        x = await __dp_asynccontextmanager_aenter(cm)
        _dp_with_ok_2 = True
        try:
            body()
        except:
            _dp_with_ok_2 = False
            _dp_with_suppress_3 = await __dp_asynccontextmanager_aexit(
                _dp_with_exit_1, __dp_exc_info()
            )
            if __dp_not_(_dp_with_suppress_3):
                raise
        finally:
            if _dp_with_ok_2:
                await __dp_asynccontextmanager_aexit(_dp_with_exit_1, None)
            _dp_with_exit_1 = None


# -- bb --
async def _dp_bb_run_0(_dp_try_exc_1):
    _dp_try_exc_1 = _dp_try_exc_1.take()
    return __dp_raise_(_dp_try_exc_1)


async def _dp_bb_run_1(_dp_try_exc_1):
    _dp_try_exc_1 = _dp_try_exc_1.take()
    _dp_with_exit_1 = __dp_NONE
    return __dp_brif(
        __dp_is_not(_dp_try_exc_1, __dp_NONE),
        _dp_bb_run_0,
        (locals().get("_dp_try_exc_1", __dp_DELETED),),
        _dp_bb_run_9,
        (),
    )


async def _dp_bb_run_2(_dp_try_exc_1, _dp_with_exit_1):
    _dp_try_exc_1, _dp_with_exit_1 = _dp_try_exc_1.take(), _dp_with_exit_1.take()
    await __dp_asynccontextmanager_aexit(_dp_with_exit_1, None)
    return __dp_jump(_dp_bb_run_1, (locals().get("_dp_try_exc_1", __dp_DELETED),))


async def _dp_bb_run_3(_dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_1):
    _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_1 = (
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_1.take(),
    )
    return __dp_brif(
        _dp_with_ok_2,
        _dp_bb_run_2,
        (locals().get("_dp_try_exc_1", __dp_DELETED), _dp_with_exit_1),
        _dp_bb_run_1,
        (locals().get("_dp_try_exc_1", __dp_DELETED),),
    )


async def _dp_bb_run_4(_dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_9):
    _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_9 = (
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_9.take(),
    )
    _dp_try_exc_1 = __dp_NONE
    return __dp_jump(
        _dp_bb_run_3,
        (_dp_with_exit_1, _dp_with_ok_2, locals().get("_dp_try_exc_1", __dp_DELETED)),
    )


async def _dp_bb_run_5(_dp_try_exc_1, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_9):
    _dp_try_exc_1, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_9 = (
        _dp_try_exc_1.take(),
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_9.take(),
    )
    body()
    return __dp_jump(
        _dp_bb_run_4,
        (_dp_with_exit_1, _dp_with_ok_2, locals().get("_dp_try_exc_9", __dp_DELETED)),
    )


async def _dp_bb_run_6(_dp_with_exit_1, _dp_with_ok_2):
    _dp_with_exit_1, _dp_with_ok_2 = _dp_with_exit_1.take(), _dp_with_ok_2.take()
    _dp_try_exc_1 = __dp_NONE
    _dp_try_exc_9 = __dp_DELETED
    return __dp_jump(
        _dp_bb_run_3,
        (_dp_with_exit_1, _dp_with_ok_2, locals().get("_dp_try_exc_1", __dp_DELETED)),
    )


async def _dp_bb_run_7(_dp_try_exc_1, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_9):
    _dp_try_exc_1, _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_9 = (
        _dp_try_exc_1.take(),
        _dp_with_exit_1.take(),
        _dp_with_ok_2.take(),
        _dp_try_exc_9.take(),
    )
    return __dp_raise_(_dp_try_exc_9)


async def _dp_bb_run_8(_dp_try_exc_1, _dp_with_exit_1, _dp_try_exc_9):
    _dp_try_exc_1, _dp_with_exit_1, _dp_try_exc_9 = (
        _dp_try_exc_1.take(),
        _dp_with_exit_1.take(),
        _dp_try_exc_9.take(),
    )
    _dp_with_ok_2 = __dp_FALSE
    _dp_with_suppress_3 = await __dp_asynccontextmanager_aexit(
        _dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_9)
    )
    return __dp_brif(
        __dp_not_(_dp_with_suppress_3),
        _dp_bb_run_7,
        (
            locals().get("_dp_try_exc_1", __dp_DELETED),
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_9", __dp_DELETED),
        ),
        _dp_bb_run_6,
        (_dp_with_exit_1, _dp_with_ok_2),
    )


async def _dp_bb_run_start():
    _dp_with_exit_1 = __dp_asynccontextmanager_get_aexit(cm)
    x = await __dp_asynccontextmanager_aenter(cm)
    _dp_with_ok_2 = __dp_TRUE
    return __dp_jump(
        _dp_bb_run_5,
        (
            locals().get("_dp_try_exc_1", __dp_DELETED),
            _dp_with_exit_1,
            _dp_with_ok_2,
            locals().get("_dp_try_exc_9", __dp_DELETED),
        ),
    )


async def _dp_bb_run_9():
    return __dp_ret(None)


__dp_setattr(_dp_bb_run_4, "_dp_exc_target", _dp_bb_run_8)
__dp_setattr(_dp_bb_run_4, "_dp_exc_name", "_dp_try_exc_9")
__dp_setattr(_dp_bb_run_5, "_dp_exc_target", _dp_bb_run_8)
__dp_setattr(_dp_bb_run_5, "_dp_exc_name", "_dp_try_exc_9")
__dp_setattr(_dp_bb_run_6, "_dp_exc_target", _dp_bb_run_3)
__dp_setattr(_dp_bb_run_6, "_dp_exc_name", "_dp_try_exc_1")
__dp_setattr(_dp_bb_run_7, "_dp_exc_target", _dp_bb_run_3)
__dp_setattr(_dp_bb_run_7, "_dp_exc_name", "_dp_try_exc_1")
__dp_setattr(_dp_bb_run_8, "_dp_exc_target", _dp_bb_run_3)
__dp_setattr(_dp_bb_run_8, "_dp_exc_name", "_dp_try_exc_1")


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"run"),
        __dp_def_coro(
            _dp_bb_run_start,
            __dp_decode_literal_bytes(b"run"),
            __dp_decode_literal_bytes(b"run"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# match_simple

match value:
    case 1:
        one()
    case _:
        other()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_match_1 = value
    if __dp_eq(_dp_match_1, 1):
        one()
    else:
        other()


# -- bb --
def _dp_bb__dp_module_init_0():
    one()
    return __dp_ret(None)


def _dp_bb__dp_module_init_1():
    other()
    return __dp_ret(None)


def _dp_bb__dp_module_init_start():
    _dp_match_1 = value
    return __dp_brif(
        __dp_eq(_dp_match_1, 1),
        _dp_bb__dp_module_init_0,
        (),
        _dp_bb__dp_module_init_1,
        (),
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

# generator_yield


def gen():
    yield 1


# ==


# -- pre-bb --
def _dp_module_init():

    def gen():
        yield 1


# -- bb --
def _dp_bb_gen_done(_dp_self, _dp_send_value, _dp_resume_exc):
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


def _dp_bb_gen_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
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


def _dp_bb_gen_uncaught(_dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_2):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_2 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_2.take(),
    )
    return __dp_brif(
        __dp_ne(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb_gen_uncaught_set_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_2",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_2"),
            ),
        ),
        _dp_bb_gen_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_2",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_2"),
            ),
        ),
    )


def _dp_bb_gen_uncaught_set_done(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_2
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_2 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_2.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_2)
    return __dp_jump(
        _dp_bb_gen_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_2",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_2"),
            ),
        ),
    )


def _dp_bb_gen_uncaught_raise(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_2
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_2 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_2.take(),
    )
    return __dp_raise_(_dp_uncaught_exc_2)


def _dp_bb_gen_resume_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    _dp_target_1 = __dp_is_(_dp_resume_exc, None)
    if _dp_target_1:
        _dp_target_1 = __dp_is_not(_dp_send_value, None)
    if _dp_target_1:
        return __dp_raise_(
            TypeError("can't send non-None value to a just-started generator")
        )
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"_pc"), 1)
    return __dp_ret(1)


def _dp_bb_gen_resume_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb_gen_dispatch_throw_done(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_gen_dispatch_throw_unstarted(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_gen_dispatch_send_target_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_resume_0, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_throw_target_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_gen_dispatch_throw_unstarted, (_dp_self, _dp_send_value, _dp_resume_exc)
    )


def _dp_bb_gen_dispatch_send_target_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_resume_1, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_throw_target_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_resume_1, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_invalid, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_send_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (_dp_bb_gen_dispatch_send_target_0, _dp_bb_gen_dispatch_send_target_1),
        _dp_bb_gen_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch_throw_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (_dp_bb_gen_dispatch_throw_target_0, _dp_bb_gen_dispatch_throw_target_1),
        _dp_bb_gen_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch_send(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_gen_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_dispatch_send_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch_throw(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_gen_dispatch_throw_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_dispatch_throw_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_is_(_dp_resume_exc, __dp_NONE),
        _dp_bb_gen_dispatch_send,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_dispatch_throw,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


__dp_setattr(_dp_bb_gen_resume_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_resume_0, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_resume_1, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_resume_1, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_send_target_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send_target_0, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_throw_target_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw_target_0, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_send_target_1, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send_target_1, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_throw_target_1, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw_target_1, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_invalid, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_invalid, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_send_table, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send_table, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_throw_table, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw_table, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_send, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch_throw, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw, "_dp_exc_name", "_dp_uncaught_exc_2")
__dp_setattr(_dp_bb_gen_dispatch, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch, "_dp_exc_name", "_dp_uncaught_exc_2")


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"gen"),
        __dp_def_gen(
            _dp_bb_gen_dispatch,
            __dp_decode_literal_bytes(b"gen"),
            __dp_decode_literal_bytes(b"gen"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# yield_from


def gen():
    yield from it


# ==


# -- pre-bb --
def _dp_module_init():

    def gen():
        yield from it


# -- bb --
def _dp_bb_gen_done(_dp_self, _dp_send_value, _dp_resume_exc):
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


def _dp_bb_gen_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
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


def _dp_bb_gen_uncaught(_dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_29):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_29 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_29.take(),
    )
    return __dp_brif(
        __dp_ne(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb_gen_uncaught_set_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_29",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_29"),
            ),
        ),
        _dp_bb_gen_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_29",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_29"),
            ),
        ),
    )


def _dp_bb_gen_uncaught_set_done(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_29
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_29 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_29.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_29)
    return __dp_jump(
        _dp_bb_gen_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_uncaught_exc_29",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_29"),
            ),
        ),
    )


def _dp_bb_gen_uncaught_raise(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_29
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_uncaught_exc_29 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_uncaught_exc_29.take(),
    )
    return __dp_raise_(_dp_uncaught_exc_29)


def _dp_bb_gen_internal_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    _dp_yield_from_iter_1 = iter(it)
    __dp_setattr(
        _dp_self, __dp_decode_literal_bytes(b"gi_yieldfrom"), _dp_yield_from_iter_1
    )
    return __dp_jump(
        _dp_bb_gen_internal_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_stop_4",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_stop_4"),
            ),
        ),
    )


def _dp_bb_gen_internal_1(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_stop_4,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_stop_4,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_stop_4.take(),
    )
    _dp_yield_from_y_2 = next(_dp_yield_from_iter_1)
    return __dp_jump(
        _dp_bb_gen_internal_6,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_y_2",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_y_2"),
            ),
        ),
    )


def _dp_bb_gen_internal_2(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_stop_4
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_stop_4 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_stop_4.take(),
    )
    return __dp_brif(
        __dp_exception_matches(_dp_yield_from_stop_4, StopIteration),
        _dp_bb_gen_internal_3,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_internal_5,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_stop_4",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_stop_4"),
            ),
        ),
    )


def _dp_bb_gen_internal_3(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_internal_4, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_internal_4(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"gi_yieldfrom"), __dp_NONE)
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb_gen_internal_5(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_stop_4
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_stop_4 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_stop_4.take(),
    )
    _dp_yield_from_raise_6 = _dp_yield_from_stop_4
    return __dp_jump(
        _dp_bb_gen_internal_11,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_raise_6",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_raise_6"),
            ),
        ),
    )


def _dp_bb_gen_internal_6(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1, _dp_yield_from_y_2
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_y_2,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_y_2.take(),
    )
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"_pc"), 1)
    __dp_store_local(
        _dp_self,
        __dp_decode_literal_bytes(b"_dp_yield_from_iter_1"),
        _dp_yield_from_iter_1,
    )
    return __dp_ret(_dp_yield_from_y_2)


def _dp_bb_gen_resume_0(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
    )
    _dp_yield_from_sent_3 = _dp_send_value
    _dp_yield_from_exc_5 = _dp_resume_exc
    _dp_resume_exc = __dp_NONE
    return __dp_brif(
        __dp_is_not(_dp_yield_from_exc_5, __dp_NONE),
        _dp_bb_gen_internal_7,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
        ),
        _dp_bb_gen_internal_15,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_sent_3",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_sent_3"),
            ),
        ),
    )


def _dp_bb_gen_internal_7(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_exc_5,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_exc_5.take(),
    )
    return __dp_brif(
        __dp_exception_matches(_dp_yield_from_exc_5, GeneratorExit),
        _dp_bb_gen_internal_8,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
        ),
        _dp_bb_gen_internal_12,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
        ),
    )


def _dp_bb_gen_internal_8(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_exc_5,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_exc_5.take(),
    )
    _dp_yield_from_close_7 = getattr(
        _dp_yield_from_iter_1, __dp_decode_literal_bytes(b"close"), __dp_NONE
    )
    return __dp_brif(
        __dp_is_not(_dp_yield_from_close_7, __dp_NONE),
        _dp_bb_gen_internal_9,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
            locals().get(
                "_dp_yield_from_close_7",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_close_7"),
            ),
        ),
        _dp_bb_gen_internal_10,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
        ),
    )


def _dp_bb_gen_internal_9(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_exc_5,
    _dp_yield_from_close_7,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_exc_5,
        _dp_yield_from_close_7,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_exc_5.take(),
        _dp_yield_from_close_7.take(),
    )
    _dp_yield_from_close_7()
    return __dp_jump(
        _dp_bb_gen_internal_10,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
        ),
    )


def _dp_bb_gen_internal_10(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_exc_5
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_exc_5 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_exc_5.take(),
    )
    _dp_yield_from_raise_6 = _dp_yield_from_exc_5
    return __dp_jump(
        _dp_bb_gen_internal_11,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_raise_6",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_raise_6"),
            ),
        ),
    )


def _dp_bb_gen_internal_11(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_raise_6
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_yield_from_raise_6 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_raise_6.take(),
    )
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"gi_yieldfrom"), __dp_NONE)
    return __dp_raise_(_dp_yield_from_raise_6)


def _dp_bb_gen_internal_12(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_exc_5,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_exc_5.take(),
    )
    _dp_yield_from_throw_8 = getattr(
        _dp_yield_from_iter_1, __dp_decode_literal_bytes(b"throw"), __dp_NONE
    )
    return __dp_brif(
        __dp_is_(_dp_yield_from_throw_8, __dp_NONE),
        _dp_bb_gen_internal_10,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
        ),
        _dp_bb_gen_internal_13,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
            locals().get(
                "_dp_yield_from_throw_8",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_throw_8"),
            ),
        ),
    )


def _dp_bb_gen_internal_13(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_exc_5,
    _dp_yield_from_throw_8,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
        _dp_yield_from_throw_8,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_exc_5.take(),
        _dp_yield_from_throw_8.take(),
    )
    return __dp_jump(
        _dp_bb_gen_internal_14,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_stop_4",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_stop_4"),
            ),
            locals().get(
                "_dp_yield_from_exc_5",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_exc_5"),
            ),
            locals().get(
                "_dp_yield_from_throw_8",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_throw_8"),
            ),
        ),
    )


def _dp_bb_gen_internal_14(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_stop_4,
    _dp_yield_from_exc_5,
    _dp_yield_from_throw_8,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_stop_4,
        _dp_yield_from_exc_5,
        _dp_yield_from_throw_8,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_stop_4.take(),
        _dp_yield_from_exc_5.take(),
        _dp_yield_from_throw_8.take(),
    )
    _dp_yield_from_y_2 = _dp_yield_from_throw_8(_dp_yield_from_exc_5)
    return __dp_jump(
        _dp_bb_gen_internal_6,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_y_2",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_y_2"),
            ),
        ),
    )


def _dp_bb_gen_internal_15(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_sent_3,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_sent_3,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_sent_3.take(),
    )
    return __dp_jump(
        _dp_bb_gen_internal_16,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_stop_4",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_stop_4"),
            ),
            locals().get(
                "_dp_yield_from_sent_3",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_sent_3"),
            ),
        ),
    )


def _dp_bb_gen_internal_16(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_stop_4,
    _dp_yield_from_sent_3,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_stop_4,
        _dp_yield_from_sent_3,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_stop_4.take(),
        _dp_yield_from_sent_3.take(),
    )
    return __dp_brif(
        __dp_is_(_dp_yield_from_sent_3, __dp_NONE),
        _dp_bb_gen_internal_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_stop_4",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_stop_4"),
            ),
        ),
        _dp_bb_gen_internal_17,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_stop_4",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_stop_4"),
            ),
            locals().get(
                "_dp_yield_from_sent_3",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_sent_3"),
            ),
        ),
    )


def _dp_bb_gen_internal_17(
    _dp_self,
    _dp_send_value,
    _dp_resume_exc,
    _dp_yield_from_iter_1,
    _dp_yield_from_stop_4,
    _dp_yield_from_sent_3,
):
    (
        _dp_self,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_stop_4,
        _dp_yield_from_sent_3,
    ) = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_yield_from_iter_1.take(),
        _dp_yield_from_stop_4.take(),
        _dp_yield_from_sent_3.take(),
    )
    _dp_yield_from_y_2 = __dp_getattr(
        _dp_yield_from_iter_1, __dp_decode_literal_bytes(b"send")
    )(_dp_yield_from_sent_3)
    return __dp_jump(
        _dp_bb_gen_internal_6,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
            locals().get(
                "_dp_yield_from_y_2",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_y_2"),
            ),
        ),
    )


def _dp_bb_gen_resume_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    _dp_target_1 = __dp_is_(_dp_resume_exc, None)
    if _dp_target_1:
        _dp_target_1 = __dp_is_not(_dp_send_value, None)
    if _dp_target_1:
        return __dp_raise_(
            TypeError("can't send non-None value to a just-started generator")
        )
    return __dp_jump(_dp_bb_gen_internal_0, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_throw_done(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_gen_dispatch_throw_unstarted(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_gen_dispatch_send_target_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_resume_1, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_throw_target_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_gen_dispatch_throw_unstarted, (_dp_self, _dp_send_value, _dp_resume_exc)
    )


def _dp_bb_gen_dispatch_send_target_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_gen_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
        ),
    )


def _dp_bb_gen_dispatch_throw_target_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_gen_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_yield_from_iter_1",
                __dp_load_local_raw(_dp_self, "_dp_yield_from_iter_1"),
            ),
        ),
    )


def _dp_bb_gen_dispatch_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(_dp_bb_gen_invalid, (_dp_self, _dp_send_value, _dp_resume_exc))


def _dp_bb_gen_dispatch_send_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (_dp_bb_gen_dispatch_send_target_0, _dp_bb_gen_dispatch_send_target_1),
        _dp_bb_gen_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch_throw_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (_dp_bb_gen_dispatch_throw_target_0, _dp_bb_gen_dispatch_throw_target_1),
        _dp_bb_gen_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch_send(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_gen_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_dispatch_send_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch_throw(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_gen_dispatch_throw_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_dispatch_throw_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_gen_dispatch(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_is_(_dp_resume_exc, __dp_NONE),
        _dp_bb_gen_dispatch_send,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_dispatch_throw,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


__dp_setattr(_dp_bb_gen_internal_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_0, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_1, "_dp_exc_target", _dp_bb_gen_internal_2)
__dp_setattr(_dp_bb_gen_internal_1, "_dp_exc_name", "_dp_yield_from_stop_4")
__dp_setattr(_dp_bb_gen_internal_2, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_2, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_3, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_3, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_4, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_4, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_5, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_5, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_6, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_6, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_resume_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_resume_0, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_7, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_7, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_8, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_8, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_9, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_9, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_10, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_10, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_11, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_11, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_12, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_12, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_13, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_13, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_14, "_dp_exc_target", _dp_bb_gen_internal_2)
__dp_setattr(_dp_bb_gen_internal_14, "_dp_exc_name", "_dp_yield_from_stop_4")
__dp_setattr(_dp_bb_gen_internal_15, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_internal_15, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_internal_16, "_dp_exc_target", _dp_bb_gen_internal_2)
__dp_setattr(_dp_bb_gen_internal_16, "_dp_exc_name", "_dp_yield_from_stop_4")
__dp_setattr(_dp_bb_gen_internal_17, "_dp_exc_target", _dp_bb_gen_internal_2)
__dp_setattr(_dp_bb_gen_internal_17, "_dp_exc_name", "_dp_yield_from_stop_4")
__dp_setattr(_dp_bb_gen_resume_1, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_resume_1, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_send_target_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send_target_0, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_throw_target_0, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw_target_0, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_send_target_1, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send_target_1, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_throw_target_1, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw_target_1, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_invalid, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_invalid, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_send_table, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send_table, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_throw_table, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw_table, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_send, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_send, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch_throw, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch_throw, "_dp_exc_name", "_dp_uncaught_exc_29")
__dp_setattr(_dp_bb_gen_dispatch, "_dp_exc_target", _dp_bb_gen_uncaught)
__dp_setattr(_dp_bb_gen_dispatch, "_dp_exc_name", "_dp_uncaught_exc_29")


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"gen"),
        __dp_def_gen(
            _dp_bb_gen_dispatch,
            __dp_decode_literal_bytes(b"gen"),
            __dp_decode_literal_bytes(b"gen"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_3 = Suppress()
    _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_3)
    __dp_contextmanager_enter(_dp_tmp_3)
    _dp_with_ok_2 = True
    try:
        raise RuntimeError(__dp_decode_literal_bytes(b"boom"))
    except:
        if __dp_exception_matches(__dp_current_exception(), BaseException):
            _dp_with_ok_2 = False
            __dp_contextmanager_exit(_dp_with_exit_1, __dp_exc_info())
        else:
            raise
    if _dp_with_ok_2:
        __dp_contextmanager_exit(_dp_with_exit_1, None)
    _dp_with_exit_1 = None
    _dp_tmp_3 = None


# -- bb --
def _dp_bb__dp_module_init_0():
    _dp_with_exit_1 = __dp_NONE
    _dp_tmp_3 = __dp_NONE
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(_dp_with_exit_1):
    _dp_with_exit_1 = _dp_with_exit_1.take()
    __dp_contextmanager_exit(_dp_with_exit_1, __dp_NONE)
    return __dp_jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_with_exit_1, _dp_with_ok_2):
    _dp_with_exit_1, _dp_with_ok_2 = _dp_with_exit_1.take(), _dp_with_ok_2.take()
    return __dp_brif(
        _dp_with_ok_2,
        _dp_bb__dp_module_init_1,
        (_dp_with_exit_1,),
        _dp_bb__dp_module_init_0,
        (),
    )


def _dp_bb__dp_module_init_3(_dp_with_exit_1, _dp_try_exc_7):
    _dp_with_exit_1, _dp_try_exc_7 = _dp_with_exit_1.take(), _dp_try_exc_7.take()
    return __dp_raise_(RuntimeError(__dp_decode_literal_bytes(b"boom")))


def _dp_bb__dp_module_init_4(_dp_with_exit_1, _dp_with_ok_2):
    _dp_with_exit_1, _dp_with_ok_2 = _dp_with_exit_1.take(), _dp_with_ok_2.take()
    _dp_try_exc_7 = __dp_DELETED
    return __dp_jump(_dp_bb__dp_module_init_2, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_5(_dp_with_exit_1, _dp_try_exc_7):
    _dp_with_exit_1, _dp_try_exc_7 = _dp_with_exit_1.take(), _dp_try_exc_7.take()
    _dp_with_ok_2 = __dp_FALSE
    __dp_contextmanager_exit(
        _dp_with_exit_1, __dp_exc_info_from_exception(_dp_try_exc_7)
    )
    return __dp_jump(_dp_bb__dp_module_init_4, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_6(_dp_try_exc_7):
    _dp_try_exc_7 = _dp_try_exc_7.take()
    return __dp_raise_(_dp_try_exc_7)


def _dp_bb__dp_module_init_7(_dp_with_exit_1, _dp_try_exc_7):
    _dp_with_exit_1, _dp_try_exc_7 = _dp_with_exit_1.take(), _dp_try_exc_7.take()
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_7, BaseException),
        _dp_bb__dp_module_init_5,
        (_dp_with_exit_1, locals().get("_dp_try_exc_7", __dp_DELETED)),
        _dp_bb__dp_module_init_6,
        (locals().get("_dp_try_exc_7", __dp_DELETED),),
    )


def _dp_bb__dp_module_init_start():
    _dp_tmp_3 = Suppress()
    _dp_with_exit_1 = __dp_contextmanager_get_exit(_dp_tmp_3)
    __dp_contextmanager_enter(_dp_tmp_3)
    _dp_with_ok_2 = __dp_TRUE
    return __dp_jump(
        _dp_bb__dp_module_init_3,
        (_dp_with_exit_1, locals().get("_dp_try_exc_7", __dp_DELETED)),
    )


__dp_setattr(_dp_bb__dp_module_init_3, "_dp_exc_target", _dp_bb__dp_module_init_7)
__dp_setattr(_dp_bb__dp_module_init_3, "_dp_exc_name", "_dp_try_exc_7")
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

# closure_cell_simple


def outer():
    x = 5

    def inner():
        return x

    return inner()


# ==


# -- pre-bb --
def _dp_module_init():

    def outer():
        _dp_cell_x = __dp_make_cell()
        __dp_store_cell(_dp_cell_x, 5)

        def inner():
            return __dp_load_cell(_dp_cell_x)

        return inner()


# -- bb --
def _dp_bb_inner_start(_dp_cell_x):
    _dp_cell_x = _dp_cell_x.take()
    return __dp_ret(__dp_load_cell(_dp_cell_x))


def _dp_bb_outer_start():
    _dp_cell_x = __dp_make_cell()
    __dp_store_cell(_dp_cell_x, 5)
    inner = __dp_def_fn(
        _dp_bb_inner_start,
        __dp_decode_literal_bytes(b"inner"),
        __dp_decode_literal_bytes(b"outer.<locals>.inner"),
        (("_dp_cell_x", _dp_cell_x),),
        (),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    return __dp_ret(inner())


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"outer"),
        __dp_def_fn(
            _dp_bb_outer_start,
            __dp_decode_literal_bytes(b"outer"),
            __dp_decode_literal_bytes(b"outer"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# bb_if_else_function


def choose(a, b):
    total = a + b
    if total > 5:
        return a
    else:
        return b


# ==


# -- pre-bb --
def _dp_module_init():

    def choose(a, b):
        total = __dp_add(a, b)
        if __dp_gt(total, 5):
            return a
        else:
            return b


# -- bb --
def _dp_bb_choose_0(a):
    a = a.take()
    return __dp_ret(a)


def _dp_bb_choose_1(b):
    b = b.take()
    return __dp_ret(b)


def _dp_bb_choose_start(a, b):
    a, b = a.take(), b.take()
    total = __dp_add(a, b)
    return __dp_brif(__dp_gt(total, 5), _dp_bb_choose_0, (a,), _dp_bb_choose_1, (b,))


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"choose"),
        __dp_def_fn(
            _dp_bb_choose_start,
            __dp_decode_literal_bytes(b"choose"),
            __dp_decode_literal_bytes(b"choose"),
            ("a", "b"),
            (("a", None, __dp__.NO_DEFAULT), ("b", None, __dp__.NO_DEFAULT)),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# closure_cell_nonlocal


def outer():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner()


# ==


# -- pre-bb --
def _dp_module_init():

    def outer():
        _dp_cell_x = __dp_make_cell()
        __dp_store_cell(_dp_cell_x, 5)

        def inner():
            nonlocal _dp_cell_x
            __dp_store_cell(_dp_cell_x, 2)
            return __dp_load_cell(_dp_cell_x)

        return inner()


# -- bb --
def _dp_bb_inner_start(_dp_cell_x):
    _dp_cell_x = _dp_cell_x.take()
    __dp_store_cell(_dp_cell_x, 2)
    return __dp_ret(__dp_load_cell(_dp_cell_x))


def _dp_bb_outer_start():
    _dp_cell_x = __dp_make_cell()
    __dp_store_cell(_dp_cell_x, 5)
    inner = __dp_def_fn(
        _dp_bb_inner_start,
        __dp_decode_literal_bytes(b"inner"),
        __dp_decode_literal_bytes(b"outer.<locals>.inner"),
        (("_dp_cell_x", _dp_cell_x),),
        (),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    return __dp_ret(inner())


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"outer"),
        __dp_def_fn(
            _dp_bb_outer_start,
            __dp_decode_literal_bytes(b"outer"),
            __dp_decode_literal_bytes(b"outer"),
            (),
            (),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# plain try / catch

try:
    print(1)
except Exception:
    print(2)

# ==


# -- pre-bb --
def _dp_module_init():
    try:
        print(1)
    except:
        if __dp_exception_matches(__dp_current_exception(), Exception):
            print(2)
        else:
            raise


# -- bb --
def _dp_bb__dp_module_init_0(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    return __dp_ret(None)


def _dp_bb__dp_module_init_1(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    print(1)
    return __dp_jump(
        _dp_bb__dp_module_init_0, (locals().get("_dp_try_exc_4", __dp_DELETED),)
    )


def _dp_bb__dp_module_init_2():
    _dp_try_exc_4 = __dp_DELETED
    return __dp_ret(None)


def _dp_bb__dp_module_init_3():
    print(2)
    return __dp_jump(_dp_bb__dp_module_init_2, ())


def _dp_bb__dp_module_init_4(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    return __dp_raise_(_dp_try_exc_4)


def _dp_bb__dp_module_init_5(_dp_try_exc_4):
    _dp_try_exc_4 = _dp_try_exc_4.take()
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_4, Exception),
        _dp_bb__dp_module_init_3,
        (),
        _dp_bb__dp_module_init_4,
        (locals().get("_dp_try_exc_4", __dp_DELETED),),
    )


def _dp_bb__dp_module_init_start():
    return __dp_jump(
        _dp_bb__dp_module_init_1, (locals().get("_dp_try_exc_4", __dp_DELETED),)
    )


__dp_setattr(_dp_bb__dp_module_init_0, "_dp_exc_target", _dp_bb__dp_module_init_5)
__dp_setattr(_dp_bb__dp_module_init_0, "_dp_exc_name", "_dp_try_exc_4")
__dp_setattr(_dp_bb__dp_module_init_1, "_dp_exc_target", _dp_bb__dp_module_init_5)
__dp_setattr(_dp_bb__dp_module_init_1, "_dp_exc_name", "_dp_try_exc_4")
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

# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")
    else:
        print("finsihed")


# ==


# -- pre-bb --
def _dp_module_init():

    def complicated(a):
        _dp_iter_1 = __dp_iter(a)
        _dp_completed_3 = False
        while __dp_not_(_dp_completed_3):
            _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
            if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
                _dp_completed_3 = True
            else:
                i = _dp_tmp_2
                _dp_tmp_2 = None
                try:
                    j = __dp_add(i, 1)
                    yield j
                except:
                    if __dp_exception_matches(__dp_current_exception(), Exception):
                        print(__dp_decode_literal_bytes(b"oops"))
                    else:
                        raise
        else:
            print(__dp_decode_literal_bytes(b"finsihed"))


# -- bb --
def _dp_bb_complicated_done(_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb_complicated_invalid(_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
    )
    return __dp_raise_(
        RuntimeError(
            __dp_getattr(
                __dp_decode_literal_bytes(b"invalid generator pc: {}"),
                __dp_decode_literal_bytes(b"format"),
            )(__dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")))
        )
    )


def _dp_bb_complicated_uncaught(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_uncaught_exc_15
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_uncaught_exc_15 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
        _dp_uncaught_exc_15.take(),
    )
    return __dp_brif(
        __dp_ne(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb_complicated_uncaught_set_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
            locals().get(
                "_dp_uncaught_exc_15",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_15"),
            ),
        ),
        _dp_bb_complicated_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
            locals().get(
                "_dp_uncaught_exc_15",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_15"),
            ),
        ),
    )


def _dp_bb_complicated_uncaught_set_done(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_uncaught_exc_15
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_uncaught_exc_15 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
        _dp_uncaught_exc_15.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_15)
    return __dp_jump(
        _dp_bb_complicated_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
            locals().get(
                "_dp_uncaught_exc_15",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_15"),
            ),
        ),
    )


def _dp_bb_complicated_uncaught_raise(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_uncaught_exc_15
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_uncaught_exc_15 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
        _dp_uncaught_exc_15.take(),
    )
    return __dp_raise_(_dp_uncaught_exc_15)


def _dp_bb_complicated_internal_0(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
    )
    print(__dp_decode_literal_bytes(b"finsihed"))
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb_complicated_resume_0(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_iter_2
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8, _dp_iter_2 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
        _dp_iter_2.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    return __dp_jump(
        _dp_bb_complicated_internal_8,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_internal_1(
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_8, _dp_iter_2
):
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_8, _dp_iter_2 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        i.take(),
        _dp_try_exc_8.take(),
        _dp_iter_2.take(),
    )
    j = __dp_add(i, 1)
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"_pc"), 1)
    __dp_store_local(
        _dp_self, __dp_decode_literal_bytes(b"_dp_try_exc_8"), _dp_try_exc_8
    )
    __dp_store_local(_dp_self, __dp_decode_literal_bytes(b"_dp_iter_2"), _dp_iter_2)
    return __dp_ret(j)


def _dp_bb_complicated_internal_2(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_2.take(),
        _dp_try_exc_8.take(),
    )
    _dp_try_exc_8 = __dp_DELETED
    return __dp_jump(
        _dp_bb_complicated_internal_8,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_internal_3(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_2.take(),
        _dp_try_exc_8.take(),
    )
    print(__dp_decode_literal_bytes(b"oops"))
    return __dp_jump(
        _dp_bb_complicated_internal_2,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_internal_4(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_8.take(),
    )
    return __dp_raise_(_dp_try_exc_8)


def _dp_bb_complicated_internal_5(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_2.take(),
        _dp_try_exc_8.take(),
    )
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_8, Exception),
        _dp_bb_complicated_internal_3,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
        _dp_bb_complicated_internal_4,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_internal_6(
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_iter_2, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_iter_2, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        i.take(),
        _dp_iter_2.take(),
        _dp_try_exc_8.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_internal_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("i", __dp_load_local_raw(_dp_self, "i")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
        ),
    )


def _dp_bb_complicated_internal_7(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_tmp_3, _dp_iter_2, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_tmp_3, _dp_iter_2, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_tmp_3.take(),
        _dp_iter_2.take(),
        _dp_try_exc_8.take(),
    )
    i = _dp_tmp_3
    _dp_tmp_3 = __dp_NONE
    return __dp_jump(
        _dp_bb_complicated_internal_6,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("i", __dp_load_local_raw(_dp_self, "i")),
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_internal_8(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_2, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_2.take(),
        _dp_try_exc_8.take(),
    )
    _dp_tmp_3 = __dp_next_or_sentinel(_dp_iter_2)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_3, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb_complicated_internal_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
        _dp_bb_complicated_internal_7,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_tmp_3", __dp_load_local_raw(_dp_self, "_dp_tmp_3")),
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_resume_1(
    _dp_self, _dp_send_value, _dp_resume_exc, a, _dp_try_exc_8
):
    _dp_self, _dp_send_value, _dp_resume_exc, a, _dp_try_exc_8 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        a.take(),
        _dp_try_exc_8.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    _dp_target_1 = __dp_is_(_dp_resume_exc, None)
    if _dp_target_1:
        _dp_target_1 = __dp_is_not(_dp_send_value, None)
    if _dp_target_1:
        return __dp_raise_(
            TypeError("can't send non-None value to a just-started generator")
        )
    _dp_try_exc_8 = __dp_DELETED
    _dp_iter_2 = __dp_iter(a)
    return __dp_jump(
        _dp_bb_complicated_internal_8,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_dispatch_throw_done(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_complicated_dispatch_throw_unstarted(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_complicated_dispatch_send_target_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_resume_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("a", __dp_load_local_raw(_dp_self, "a")),
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_dispatch_throw_target_0(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_dispatch_throw_unstarted,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_send_target_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
        ),
    )


def _dp_bb_complicated_dispatch_throw_target_1(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
            locals().get("_dp_iter_2", __dp_load_local_raw(_dp_self, "_dp_iter_2")),
        ),
    )


def _dp_bb_complicated_dispatch_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_invalid,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
    )


def _dp_bb_complicated_dispatch_send_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (
            _dp_bb_complicated_dispatch_send_target_0,
            _dp_bb_complicated_dispatch_send_target_1,
        ),
        _dp_bb_complicated_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_throw_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (
            _dp_bb_complicated_dispatch_throw_target_0,
            _dp_bb_complicated_dispatch_throw_target_1,
        ),
        _dp_bb_complicated_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_send(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_complicated_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_8", __dp_load_local_raw(_dp_self, "_dp_try_exc_8")
            ),
        ),
        _dp_bb_complicated_dispatch_send_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_throw(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_complicated_dispatch_throw_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_complicated_dispatch_throw_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_is_(_dp_resume_exc, __dp_NONE),
        _dp_bb_complicated_dispatch_send,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_complicated_dispatch_throw,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


__dp_setattr(
    _dp_bb_complicated_internal_0, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_0, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_resume_0, "_dp_exc_target", _dp_bb_complicated_internal_5
)
__dp_setattr(_dp_bb_complicated_resume_0, "_dp_exc_name", "_dp_try_exc_8")
__dp_setattr(
    _dp_bb_complicated_internal_1, "_dp_exc_target", _dp_bb_complicated_internal_5
)
__dp_setattr(_dp_bb_complicated_internal_1, "_dp_exc_name", "_dp_try_exc_8")
__dp_setattr(
    _dp_bb_complicated_internal_2, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_2, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_internal_3, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_3, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_internal_4, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_4, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_internal_5, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_5, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_internal_6, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_6, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_internal_7, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_7, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_internal_8, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_8, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(_dp_bb_complicated_resume_1, "_dp_exc_target", _dp_bb_complicated_uncaught)
__dp_setattr(_dp_bb_complicated_resume_1, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_0,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_0, "_dp_exc_name", "_dp_uncaught_exc_15"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_0,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_0, "_dp_exc_name", "_dp_uncaught_exc_15"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_1,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_1, "_dp_exc_name", "_dp_uncaught_exc_15"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_1,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_1, "_dp_exc_name", "_dp_uncaught_exc_15"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_invalid, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_dispatch_invalid, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_dispatch_send_table,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_table, "_dp_exc_name", "_dp_uncaught_exc_15"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_table,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_table, "_dp_exc_name", "_dp_uncaught_exc_15"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_dispatch_send, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(
    _dp_bb_complicated_dispatch_throw, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_dispatch_throw, "_dp_exc_name", "_dp_uncaught_exc_15")
__dp_setattr(_dp_bb_complicated_dispatch, "_dp_exc_target", _dp_bb_complicated_uncaught)
__dp_setattr(_dp_bb_complicated_dispatch, "_dp_exc_name", "_dp_uncaught_exc_15")


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"complicated"),
        __dp_def_gen(
            _dp_bb_complicated_dispatch,
            __dp_decode_literal_bytes(b"complicated"),
            __dp_decode_literal_bytes(b"complicated"),
            ("a", "_dp_try_exc_8"),
            (("a", None, __dp__.NO_DEFAULT),),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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

# complicated generator


def complicated(a):
    for i in a:
        try:
            j = i + 1
            yield j
        except Exception:
            print("oops")


# ==


# -- pre-bb --
def _dp_module_init():

    def complicated(a):
        _dp_iter_1 = __dp_iter(a)
        while True:
            _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
            if __dp_is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
                break
            else:
                i = _dp_tmp_2
                _dp_tmp_2 = None
                try:
                    j = __dp_add(i, 1)
                    yield j
                except:
                    if __dp_exception_matches(__dp_current_exception(), Exception):
                        print(__dp_decode_literal_bytes(b"oops"))
                    else:
                        raise


# -- bb --
def _dp_bb_complicated_done(_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb_complicated_invalid(_dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    return __dp_raise_(
        RuntimeError(
            __dp_getattr(
                __dp_decode_literal_bytes(b"invalid generator pc: {}"),
                __dp_decode_literal_bytes(b"format"),
            )(__dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")))
        )
    )


def _dp_bb_complicated_uncaught(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_uncaught_exc_14
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_uncaught_exc_14 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_uncaught_exc_14.take(),
    )
    return __dp_brif(
        __dp_ne(
            __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
            __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
        ),
        _dp_bb_complicated_uncaught_set_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
            locals().get(
                "_dp_uncaught_exc_14",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_14"),
            ),
        ),
        _dp_bb_complicated_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
            locals().get(
                "_dp_uncaught_exc_14",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_14"),
            ),
        ),
    )


def _dp_bb_complicated_uncaught_set_done(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_uncaught_exc_14
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_uncaught_exc_14 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_uncaught_exc_14.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    __dp_raise_uncaught_generator_exception(_dp_uncaught_exc_14)
    return __dp_jump(
        _dp_bb_complicated_uncaught_raise,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
            locals().get(
                "_dp_uncaught_exc_14",
                __dp_load_local_raw(_dp_self, "_dp_uncaught_exc_14"),
            ),
        ),
    )


def _dp_bb_complicated_uncaught_raise(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_uncaught_exc_14
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_uncaught_exc_14 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_uncaught_exc_14.take(),
    )
    return __dp_raise_(_dp_uncaught_exc_14)


def _dp_bb_complicated_resume_0(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
        _dp_iter_1.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    return __dp_jump(
        _dp_bb_complicated_internal_7,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_internal_0(
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1
):
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_try_exc_7, _dp_iter_1 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        i.take(),
        _dp_try_exc_7.take(),
        _dp_iter_1.take(),
    )
    j = __dp_add(i, 1)
    __dp_setattr(_dp_self, __dp_decode_literal_bytes(b"_pc"), 1)
    __dp_store_local(
        _dp_self, __dp_decode_literal_bytes(b"_dp_try_exc_7"), _dp_try_exc_7
    )
    __dp_store_local(_dp_self, __dp_decode_literal_bytes(b"_dp_iter_1"), _dp_iter_1)
    return __dp_ret(j)


def _dp_bb_complicated_internal_1(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    _dp_try_exc_7 = __dp_DELETED
    return __dp_jump(
        _dp_bb_complicated_internal_7,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_internal_2(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    print(__dp_decode_literal_bytes(b"oops"))
    return __dp_jump(
        _dp_bb_complicated_internal_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_internal_3(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    return __dp_raise_(_dp_try_exc_7)


def _dp_bb_complicated_internal_4(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    return __dp_brif(
        __dp_exception_matches(_dp_try_exc_7, Exception),
        _dp_bb_complicated_internal_2,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
        _dp_bb_complicated_internal_3,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_internal_5(
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, i, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        i.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_internal_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("i", __dp_load_local_raw(_dp_self, "i")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
        ),
    )


def _dp_bb_complicated_internal_6(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_tmp_2, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_tmp_2, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_tmp_2.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    i = _dp_tmp_2
    _dp_tmp_2 = __dp_NONE
    return __dp_jump(
        _dp_bb_complicated_internal_5,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("i", __dp_load_local_raw(_dp_self, "i")),
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_internal_7(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_iter_1.take(),
        _dp_try_exc_7.take(),
    )
    _dp_tmp_2 = __dp_next_or_sentinel(_dp_iter_1)
    return __dp_brif(
        __dp_is_(
            _dp_tmp_2, __dp_getattr(__dp__, __dp_decode_literal_bytes(b"ITER_COMPLETE"))
        ),
        _dp_bb_complicated_internal_8,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
        _dp_bb_complicated_internal_6,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_tmp_2", __dp_load_local_raw(_dp_self, "_dp_tmp_2")),
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_resume_1(
    _dp_self, _dp_send_value, _dp_resume_exc, a, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, a, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        a.take(),
        _dp_try_exc_7.take(),
    )
    if __dp_is_not(_dp_resume_exc, None):
        return __dp_raise_(_dp_resume_exc)
    _dp_target_1 = __dp_is_(_dp_resume_exc, None)
    if _dp_target_1:
        _dp_target_1 = __dp_is_not(_dp_send_value, None)
    if _dp_target_1:
        return __dp_raise_(
            TypeError("can't send non-None value to a just-started generator")
        )
    _dp_try_exc_7 = __dp_DELETED
    _dp_iter_1 = __dp_iter(a)
    return __dp_jump(
        _dp_bb_complicated_internal_7,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_internal_8(
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7
):
    _dp_self, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
        _dp_try_exc_7.take(),
    )
    __dp_setattr(
        _dp_self,
        __dp_decode_literal_bytes(b"_pc"),
        __dp_getattr(__dp__, __dp_decode_literal_bytes(b"_GEN_PC_DONE")),
    )
    return __dp_raise_(StopIteration())


def _dp_bb_complicated_dispatch_throw_done(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_complicated_dispatch_throw_unstarted(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_raise_(_dp_resume_exc)


def _dp_bb_complicated_dispatch_send_target_0(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_resume_1,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get("a", __dp_load_local_raw(_dp_self, "a")),
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_dispatch_throw_target_0(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_dispatch_throw_unstarted,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_send_target_1(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
        ),
    )


def _dp_bb_complicated_dispatch_throw_target_1(
    _dp_self, _dp_send_value, _dp_resume_exc
):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_resume_0,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
            locals().get("_dp_iter_1", __dp_load_local_raw(_dp_self, "_dp_iter_1")),
        ),
    )


def _dp_bb_complicated_dispatch_invalid(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_jump(
        _dp_bb_complicated_invalid,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
    )


def _dp_bb_complicated_dispatch_send_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (
            _dp_bb_complicated_dispatch_send_target_0,
            _dp_bb_complicated_dispatch_send_target_1,
        ),
        _dp_bb_complicated_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_throw_table(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_br_table(
        __dp_getattr(_dp_self, __dp_decode_literal_bytes(b"_pc")),
        (
            _dp_bb_complicated_dispatch_throw_target_0,
            _dp_bb_complicated_dispatch_throw_target_1,
        ),
        _dp_bb_complicated_dispatch_invalid,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_send(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_complicated_done,
        (
            _dp_self,
            _dp_send_value,
            _dp_resume_exc,
            locals().get(
                "_dp_try_exc_7", __dp_load_local_raw(_dp_self, "_dp_try_exc_7")
            ),
        ),
        _dp_bb_complicated_dispatch_send_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch_throw(_dp_self, _dp_send_value, _dp_resume_exc):
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
        _dp_bb_complicated_dispatch_throw_done,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_complicated_dispatch_throw_table,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


def _dp_bb_complicated_dispatch(_dp_self, _dp_send_value, _dp_resume_exc):
    _dp_self, _dp_send_value, _dp_resume_exc = (
        _dp_self.take(),
        _dp_send_value.take(),
        _dp_resume_exc.take(),
    )
    return __dp_brif(
        __dp_is_(_dp_resume_exc, __dp_NONE),
        _dp_bb_complicated_dispatch_send,
        (_dp_self, _dp_send_value, _dp_resume_exc),
        _dp_bb_complicated_dispatch_throw,
        (_dp_self, _dp_send_value, _dp_resume_exc),
    )


__dp_setattr(
    _dp_bb_complicated_resume_0, "_dp_exc_target", _dp_bb_complicated_internal_4
)
__dp_setattr(_dp_bb_complicated_resume_0, "_dp_exc_name", "_dp_try_exc_7")
__dp_setattr(
    _dp_bb_complicated_internal_0, "_dp_exc_target", _dp_bb_complicated_internal_4
)
__dp_setattr(_dp_bb_complicated_internal_0, "_dp_exc_name", "_dp_try_exc_7")
__dp_setattr(
    _dp_bb_complicated_internal_1, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_1, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_2, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_2, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_3, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_3, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_4, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_4, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_5, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_5, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_6, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_6, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_7, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_7, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(_dp_bb_complicated_resume_1, "_dp_exc_target", _dp_bb_complicated_uncaught)
__dp_setattr(_dp_bb_complicated_resume_1, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_internal_8, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_internal_8, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_0,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_0, "_dp_exc_name", "_dp_uncaught_exc_14"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_0,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_0, "_dp_exc_name", "_dp_uncaught_exc_14"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_1,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_target_1, "_dp_exc_name", "_dp_uncaught_exc_14"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_1,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_target_1, "_dp_exc_name", "_dp_uncaught_exc_14"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_invalid, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_dispatch_invalid, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_dispatch_send_table,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send_table, "_dp_exc_name", "_dp_uncaught_exc_14"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_table,
    "_dp_exc_target",
    _dp_bb_complicated_uncaught,
)
__dp_setattr(
    _dp_bb_complicated_dispatch_throw_table, "_dp_exc_name", "_dp_uncaught_exc_14"
)
__dp_setattr(
    _dp_bb_complicated_dispatch_send, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_dispatch_send, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(
    _dp_bb_complicated_dispatch_throw, "_dp_exc_target", _dp_bb_complicated_uncaught
)
__dp_setattr(_dp_bb_complicated_dispatch_throw, "_dp_exc_name", "_dp_uncaught_exc_14")
__dp_setattr(_dp_bb_complicated_dispatch, "_dp_exc_target", _dp_bb_complicated_uncaught)
__dp_setattr(_dp_bb_complicated_dispatch, "_dp_exc_name", "_dp_uncaught_exc_14")


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"complicated"),
        __dp_def_gen(
            _dp_bb_complicated_dispatch,
            __dp_decode_literal_bytes(b"complicated"),
            __dp_decode_literal_bytes(b"complicated"),
            ("a", "_dp_try_exc_7"),
            (("a", None, __dp__.NO_DEFAULT),),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
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
