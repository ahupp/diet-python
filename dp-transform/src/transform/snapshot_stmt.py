# import_simple

import a

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "a", __dp__.import_("a", __spec__))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "a", __dp__.import_("a", __spec__))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# import_dotted_alias

import a.b as c

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(
        globals(), "c", __dp__.import_attr(__dp__.import_("a.b", __spec__), "b")
    )


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(), "c", __dp__.import_attr(__dp__.import_("a.b", __spec__), "b")
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# import_from_alias

from pkg.mod import name as alias

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_import_1 = __dp__.import_("pkg.mod", __spec__, __dp__.list(("name",)))
    __dp__.store_global(globals(), "alias", __dp__.import_attr(_dp_import_1, "name"))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_import_1 = __dp__.import_("pkg.mod", __spec__, __dp__.list(("name",)))
    __dp__.store_global(globals(), "alias", __dp__.import_attr(_dp_import_1, "name"))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
def _dp_bb_f_start(_dp_args_ptr):
    return __dp__.ret(None)


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(), "f", dec(__dp__.def_fn(_dp_bb_f_start, "f", "f", (), (), __name__))
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assign_attr

obj.x = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.setattr(obj, "x", 1)


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.setattr(obj, "x", 1)
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assign_subscript

obj[i] = v

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.setitem(obj, i, v)


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.setitem(obj, i, v)
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assign_tuple_unpack

a, b = it

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = __dp__.unpack(it, (True, True))
    __dp__.store_global(globals(), "a", __dp__.getitem(_dp_tmp_1, 0))
    __dp__.store_global(globals(), "b", __dp__.getitem(_dp_tmp_1, 1))
    del _dp_tmp_1


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_tmp_1 = __dp__.unpack(it, (True, True))
    __dp__.store_global(
        globals(),
        "a",
        __dp__.getitem(__dp__.load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0),
    )
    __dp__.store_global(
        globals(),
        "b",
        __dp__.getitem(__dp__.load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1),
    )
    _dp_tmp_1 = __dp__.DELETED
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assign_star_unpack

a, *b = it

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = __dp__.unpack(it, (True, False))
    __dp__.store_global(globals(), "a", __dp__.getitem(_dp_tmp_1, 0))
    __dp__.store_global(globals(), "b", __dp__.list(__dp__.getitem(_dp_tmp_1, 1)))
    del _dp_tmp_1


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_tmp_1 = __dp__.unpack(it, (True, False))
    __dp__.store_global(
        globals(),
        "a",
        __dp__.getitem(__dp__.load_deleted_name("_dp_tmp_1", _dp_tmp_1), 0),
    )
    __dp__.store_global(
        globals(),
        "b",
        __dp__.list(
            __dp__.getitem(__dp__.load_deleted_name("_dp_tmp_1", _dp_tmp_1), 1)
        ),
    )
    _dp_tmp_1 = __dp__.DELETED
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assign_multi_targets

a = b = f()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = f()
    __dp__.store_global(globals(), "a", _dp_tmp_1)
    __dp__.store_global(globals(), "b", _dp_tmp_1)


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_tmp_1 = f()
    __dp__.store_global(globals(), "a", _dp_tmp_1)
    __dp__.store_global(globals(), "b", _dp_tmp_1)
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# ann_assign_simple

x: int = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.store_global(globals(), "x", 1)

    def __annotate__(_dp_format, _dp=__dp__):
        if _dp.eq(_dp_format, 4):
            return __dp__.dict((("x", "int"),))
        if _dp.gt(_dp_format, 2):
            raise _dp.builtins.NotImplementedError
        return __dp__.dict((("x", int),))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(globals(), "x", 1)

    def __annotate__(_dp_format, _dp=__dp__):
        if _dp.eq(_dp_format, 4):
            return __dp__.dict((("x", "int"),))
        if _dp.gt(_dp_format, 2):
            raise _dp.builtins.NotImplementedError
        return __dp__.dict((("x", int),))

    __dp__.store_global(
        globals(),
        "__annotate__",
        __dp__.update_fn(__annotate__, "__annotate__", "__annotate__"),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# ann_assign_attr

obj.x: int = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.setattr(obj, "x", 1)


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.setattr(obj, "x", 1)
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# aug_assign_attr

obj.x += 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.setattr(obj, "x", __dp__.iadd(obj.x, 1))


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.setattr(obj, "x", __dp__.iadd(obj.x, 1))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# delete_mixed

del obj.x, obj[i], x

# ==


# -- pre-bb --
def _dp_module_init():
    __dp__.delattr(obj, "x")
    __dp__.delitem(obj, i)
    __dp__.delitem(globals(), "x")


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.delattr(obj, "x")
    __dp__.delitem(obj, i)
    __dp__.delitem(globals(), "x")
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assert_no_msg

assert cond

# ==


# -- pre-bb --
def _dp_module_init():
    if __debug__:
        if __dp__.not_(cond):
            raise __dp__.builtins.AssertionError


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    return __dp__.raise_(__dp__.builtins.AssertionError)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    return __dp__.brif(
        __dp__.not_(cond), _dp_bb__dp_module_init_0, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.brif(
        __debug__, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# assert_with_msg

assert cond, "oops"

# ==


# -- pre-bb --
def _dp_module_init():
    if __debug__:
        if __dp__.not_(cond):
            raise __dp__.builtins.AssertionError("oops")


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    return __dp__.raise_(__dp__.builtins.AssertionError("oops"))


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    return __dp__.brif(
        __dp__.not_(cond), _dp_bb__dp_module_init_0, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.brif(
        __debug__, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_2, ()
    )


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# raise_from

raise E from cause

# ==


# -- pre-bb --
def _dp_module_init():
    raise __dp__.raise_from(E, cause)


# -- bb --
def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.raise_(__dp__.raise_from(E, cause))


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        if __dp__.exception_matches(__dp__.current_exception(), E):
            __dp__.store_global(globals(), "e", __dp__.current_exception())
            try:
                g(__dp__.load_global(globals(), "e"))
            finally:
                try:
                    __dp__.delitem(globals(), "e")
                except:
                    if __dp__.exception_matches(__dp__.current_exception(), NameError):
                        pass
                    else:
                        raise
        else:
            h()


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    f()
    return __dp__.jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.DELETED
    return __dp__.ret(None)


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    return __dp__.jump(_dp_bb__dp_module_init_2, ())


def _dp_bb__dp_module_init_4(_dp_args_ptr):
    __dp__.delitem(globals(), "e")
    return __dp__.jump(_dp_bb__dp_module_init_3, ())


def _dp_bb__dp_module_init_5(_dp_args_ptr):
    _dp_try_exc_8 = __dp__.DELETED
    return __dp__.jump(_dp_bb__dp_module_init_2, ())


def _dp_bb__dp_module_init_6(_dp_args_ptr):
    return __dp__.jump(_dp_bb__dp_module_init_5, ())


def _dp_bb__dp_module_init_7(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_4)


def _dp_bb__dp_module_init_8(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    _dp_try_exc_8 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_4, NameError),
        _dp_bb__dp_module_init_6,
        (),
        _dp_bb__dp_module_init_7,
        (_dp_try_exc_4,),
    )


def _dp_bb__dp_module_init_9(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_4,
        (),
        (_dp_bb__dp_module_init_3, _dp_bb__dp_module_init_4),
        _dp_bb__dp_module_init_8,
        (_dp_try_exc_4,),
        (
            _dp_bb__dp_module_init_5,
            _dp_bb__dp_module_init_6,
            _dp_bb__dp_module_init_7,
            _dp_bb__dp_module_init_8,
        ),
        None,
        (),
        (),
        None,
    )


def _dp_bb__dp_module_init_10(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.jump(_dp_bb__dp_module_init_9, (_dp_try_exc_4,))


def _dp_bb__dp_module_init_11(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    g(__dp__.load_global(globals(), "e"))
    return __dp__.jump(_dp_bb__dp_module_init_10, (_dp_try_exc_4,))


def _dp_bb__dp_module_init_12(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    _dp_try_exc_16 = __dp__.current_exception()
    return __dp__.raise_(_dp_try_exc_16)


def _dp_bb__dp_module_init_13(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.store_global(globals(), "e", _dp_try_exc_4)
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_11,
        (_dp_try_exc_4,),
        (_dp_bb__dp_module_init_10, _dp_bb__dp_module_init_11),
        _dp_bb__dp_module_init_12,
        (_dp_try_exc_4,),
        (_dp_bb__dp_module_init_12,),
        _dp_bb__dp_module_init_9,
        (_dp_try_exc_4,),
        (
            _dp_bb__dp_module_init_3,
            _dp_bb__dp_module_init_4,
            _dp_bb__dp_module_init_5,
            _dp_bb__dp_module_init_6,
            _dp_bb__dp_module_init_7,
            _dp_bb__dp_module_init_8,
            _dp_bb__dp_module_init_9,
        ),
        _dp_bb__dp_module_init_2,
    )


def _dp_bb__dp_module_init_14(_dp_args_ptr):
    h()
    return __dp__.jump(_dp_bb__dp_module_init_2, ())


def _dp_bb__dp_module_init_15(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_4, E),
        _dp_bb__dp_module_init_13,
        (_dp_try_exc_4,),
        _dp_bb__dp_module_init_14,
        (),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_1,
        (),
        (_dp_bb__dp_module_init_0, _dp_bb__dp_module_init_1),
        _dp_bb__dp_module_init_15,
        (),
        (
            _dp_bb__dp_module_init_2,
            _dp_bb__dp_module_init_3,
            _dp_bb__dp_module_init_4,
            _dp_bb__dp_module_init_5,
            _dp_bb__dp_module_init_6,
            _dp_bb__dp_module_init_7,
            _dp_bb__dp_module_init_8,
            _dp_bb__dp_module_init_9,
            _dp_bb__dp_module_init_10,
            _dp_bb__dp_module_init_11,
            _dp_bb__dp_module_init_12,
            _dp_bb__dp_module_init_13,
            _dp_bb__dp_module_init_14,
            _dp_bb__dp_module_init_15,
        ),
        None,
        (),
        (),
        None,
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
    for x in it:
        __dp__.store_global(globals(), "x", x)
        body()
    else:
        done()


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    done()
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    x, _dp_iter_2 = __dp__.take_args(_dp_args_ptr)
    __dp__.store_global(globals(), "x", x)
    body()
    return __dp__.jump(_dp_bb__dp_module_init_3, (_dp_iter_2,))


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_tmp_3, _dp_iter_2 = __dp__.take_args(_dp_args_ptr)
    x = _dp_tmp_3
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_module_init_1, (x, _dp_iter_2))


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    _dp_iter_2 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_3 = __dp__.next_or_sentinel(_dp_iter_2)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_3, __dp__.ITER_COMPLETE),
        _dp_bb__dp_module_init_0,
        (),
        _dp_bb__dp_module_init_2,
        (_dp_tmp_3, _dp_iter_2),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_iter_2 = __dp__.iter(it)
    return __dp__.jump(_dp_bb__dp_module_init_3, (_dp_iter_2,))


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    done()
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    body()
    return __dp__.jump(_dp_bb__dp_module_init_start, ())


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.brif(cond, _dp_bb__dp_module_init_1, (), _dp_bb__dp_module_init_0, ())


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# with_as

with cm as x:
    body()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_with_exit_1 = __dp__.contextmanager_get_exit(cm)
    __dp__.store_global(globals(), "x", __dp__.contextmanager_enter(cm))
    _dp_with_ok_2 = True
    try:
        body()
    except:
        if __dp__.exception_matches(__dp__.current_exception(), BaseException):
            _dp_with_ok_2 = False
            __dp__.contextmanager_exit(_dp_with_exit_1, __dp__.exc_info())
        else:
            raise
    if _dp_with_ok_2:
        __dp__.contextmanager_exit(_dp_with_exit_1, None)
    _dp_with_exit_1 = None


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    _dp_with_exit_1 = None
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.contextmanager_exit(_dp_with_exit_1, None)
    return __dp__.jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        _dp_with_ok_2,
        _dp_bb__dp_module_init_1,
        (_dp_with_exit_1,),
        _dp_bb__dp_module_init_0,
        (),
    )


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    return __dp__.jump(_dp_bb__dp_module_init_2, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_4(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    body()
    return __dp__.jump(_dp_bb__dp_module_init_3, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_5(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.DELETED
    return __dp__.jump(_dp_bb__dp_module_init_2, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_6(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    _dp_with_ok_2 = False
    __dp__.contextmanager_exit(_dp_with_exit_1, __dp__.exc_info())
    return __dp__.jump(_dp_bb__dp_module_init_5, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_7(_dp_args_ptr):
    _dp_try_exc_7 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_7)


def _dp_bb__dp_module_init_8(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_7, BaseException),
        _dp_bb__dp_module_init_6,
        (_dp_with_exit_1,),
        _dp_bb__dp_module_init_7,
        (_dp_try_exc_7,),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.contextmanager_get_exit(cm)
    __dp__.store_global(globals(), "x", __dp__.contextmanager_enter(cm))
    _dp_with_ok_2 = True
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_4,
        (_dp_with_exit_1, _dp_with_ok_2),
        (_dp_bb__dp_module_init_3, _dp_bb__dp_module_init_4),
        _dp_bb__dp_module_init_8,
        (_dp_with_exit_1,),
        (
            _dp_bb__dp_module_init_5,
            _dp_bb__dp_module_init_6,
            _dp_bb__dp_module_init_7,
            _dp_bb__dp_module_init_8,
        ),
        None,
        (),
        (),
        None,
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
def _dp_bb_inner_start(_dp_args_ptr):
    value = 1
    return __dp__.ret(value)


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "inner",
        __dp__.def_fn(_dp_bb_inner_start, "inner", "inner", (), (), __name__),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        _dp_tmp_1 = __dp__.list(())
        for x in _dp_iter_2:
            _dp_tmp_1.append(x)
        return _dp_tmp_1

    __dp__.store_global(globals(), "xs", _dp_listcomp_3(it))

    def _dp_setcomp_6(_dp_iter_5):
        _dp_tmp_4 = set()
        for x in _dp_iter_5:
            _dp_tmp_4.add(x)
        return _dp_tmp_4

    __dp__.store_global(globals(), "ys", _dp_setcomp_6(it))

    def _dp_dictcomp_9(_dp_iter_8):
        _dp_tmp_7 = __dp__.dict()
        for k, v in _dp_iter_8:
            __dp__.setitem(_dp_tmp_7, k, v)
        return _dp_tmp_7

    __dp__.store_global(globals(), "zs", _dp_dictcomp_9(items))


# -- bb --
def _dp_bb__dp_listcomp_3_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, x = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_1.append(x)
    return __dp__.jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = __dp__.take_args(_dp_args_ptr)
    x = _dp_tmp_3
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_listcomp_3_1, (_dp_iter_2, _dp_tmp_1, x))


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


def _dp_bb__dp_setcomp_6_0(_dp_args_ptr):
    _dp_tmp_4 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_4)


def _dp_bb__dp_setcomp_6_1(_dp_args_ptr):
    _dp_tmp_4, x, _dp_iter_10 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_4.add(x)
    return __dp__.jump(_dp_bb__dp_setcomp_6_3, (_dp_tmp_4, _dp_iter_10))


def _dp_bb__dp_setcomp_6_2(_dp_args_ptr):
    _dp_tmp_4, _dp_tmp_11, _dp_iter_10 = __dp__.take_args(_dp_args_ptr)
    x = _dp_tmp_11
    _dp_tmp_11 = None
    return __dp__.jump(_dp_bb__dp_setcomp_6_1, (_dp_tmp_4, x, _dp_iter_10))


def _dp_bb__dp_setcomp_6_3(_dp_args_ptr):
    _dp_tmp_4, _dp_iter_10 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_11 = __dp__.next_or_sentinel(_dp_iter_10)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_11, __dp__.ITER_COMPLETE),
        _dp_bb__dp_setcomp_6_0,
        (_dp_tmp_4,),
        _dp_bb__dp_setcomp_6_2,
        (_dp_tmp_4, _dp_tmp_11, _dp_iter_10),
    )


def _dp_bb__dp_setcomp_6_start(_dp_args_ptr):
    _dp_iter_5 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_4 = set()
    _dp_iter_10 = __dp__.iter(_dp_iter_5)
    return __dp__.jump(_dp_bb__dp_setcomp_6_3, (_dp_tmp_4, _dp_iter_10))


def _dp_bb__dp_dictcomp_9_0(_dp_args_ptr):
    _dp_tmp_7 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_7)


def _dp_bb__dp_dictcomp_9_1(_dp_args_ptr):
    _dp_tmp_7, k, v, _dp_iter_18 = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_tmp_7, k, v)
    return __dp__.jump(_dp_bb__dp_dictcomp_9_3, (_dp_tmp_7, _dp_iter_18))


def _dp_bb__dp_dictcomp_9_2(_dp_args_ptr):
    _dp_tmp_7, _dp_tmp_19, _dp_iter_18 = __dp__.take_args(_dp_args_ptr)
    k = _dp_tmp_19[0]
    v = _dp_tmp_19[1]
    _dp_tmp_19 = None
    return __dp__.jump(_dp_bb__dp_dictcomp_9_1, (_dp_tmp_7, k, v, _dp_iter_18))


def _dp_bb__dp_dictcomp_9_3(_dp_args_ptr):
    _dp_tmp_7, _dp_iter_18 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_19 = __dp__.next_or_sentinel(_dp_iter_18)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_19, __dp__.ITER_COMPLETE),
        _dp_bb__dp_dictcomp_9_0,
        (_dp_tmp_7,),
        _dp_bb__dp_dictcomp_9_2,
        (_dp_tmp_7, _dp_tmp_19, _dp_iter_18),
    )


def _dp_bb__dp_dictcomp_9_start(_dp_args_ptr):
    _dp_iter_8 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_7 = __dp__.dict()
    _dp_iter_18 = __dp__.iter(_dp_iter_8)
    return __dp__.jump(_dp_bb__dp_dictcomp_9_3, (_dp_tmp_7, _dp_iter_18))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_listcomp_3 = __dp__.def_fn(
        _dp_bb__dp_listcomp_3_start,
        "<listcomp>",
        "_dp_listcomp_3",
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "xs", _dp_listcomp_3(it))
    _dp_setcomp_6 = __dp__.def_fn(
        _dp_bb__dp_setcomp_6_start,
        "<setcomp>",
        "_dp_setcomp_6",
        ("_dp_iter_5",),
        (("_dp_iter_5", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "ys", _dp_setcomp_6(it))
    _dp_dictcomp_9 = __dp__.def_fn(
        _dp_bb__dp_dictcomp_9_start,
        "<dictcomp>",
        "_dp_dictcomp_9",
        ("_dp_iter_8",),
        (("_dp_iter_8", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    __dp__.store_global(globals(), "zs", _dp_dictcomp_9(items))
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
            _dp_tmp_1 = __dp__.list(())
            for x in _dp_iter_2:
                if __dp__.gt(x, 0):
                    _dp_tmp_1.append(x)
            return _dp_tmp_1

        return _dp_listcomp_3(it)


# -- bb --
def _dp_bb__dp_listcomp_3_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, x = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_1.append(x)
    return __dp__.jump(_dp_bb__dp_listcomp_3_4, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, x = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        __dp__.gt(x, 0),
        _dp_bb__dp_listcomp_3_1,
        (_dp_iter_2, _dp_tmp_1, x),
        _dp_bb__dp_listcomp_3_4,
        (_dp_iter_2, _dp_tmp_1),
    )


def _dp_bb__dp_listcomp_3_3(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = __dp__.take_args(_dp_args_ptr)
    x = _dp_tmp_3
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_listcomp_3_2, (_dp_iter_2, _dp_tmp_1, x))


def _dp_bb__dp_listcomp_3_4(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1 = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_3 = __dp__.next_or_sentinel(_dp_iter_2)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_3, __dp__.ITER_COMPLETE),
        _dp_bb__dp_listcomp_3_0,
        (_dp_tmp_1,),
        _dp_bb__dp_listcomp_3_3,
        (_dp_iter_2, _dp_tmp_1, _dp_tmp_3),
    )


def _dp_bb__dp_listcomp_3_start(_dp_args_ptr):
    _dp_iter_2 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_1 = __dp__.list(())
    _dp_iter_2 = __dp__.iter(_dp_iter_2)
    return __dp__.jump(_dp_bb__dp_listcomp_3_4, (_dp_iter_2, _dp_tmp_1))


def _dp_bb_f_start(_dp_args_ptr):
    _dp_listcomp_3 = __dp__.def_fn(
        _dp_bb__dp_listcomp_3_start,
        "<listcomp>",
        "f.<locals>._dp_listcomp_3",
        ("_dp_iter_2",),
        (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
        __name__,
    )
    return __dp__.ret(_dp_listcomp_3(it))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(), "f", __dp__.def_fn(_dp_bb_f_start, "f", "f", (), (), __name__)
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "C")

        def _dp_listcomp_3(_dp_iter_2):
            _dp_tmp_1 = __dp__.list(())
            for x in _dp_iter_2:
                _dp_tmp_1.append(x)
            return _dp_tmp_1

        __dp__.setitem(
            _dp_class_ns,
            "xs",
            _dp_listcomp_3(__dp__.class_lookup_global(_dp_class_ns, "it", globals())),
        )

    def _dp_define_class_C():
        return __dp__.create_class("C", _dp_class_ns_C, (), None, False, 3, ())

    __dp__.store_global(globals(), "C", _dp_define_class_C())


# -- bb --
def _dp_bb__dp_listcomp_3_0(_dp_args_ptr):
    _dp_tmp_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(_dp_tmp_1)


def _dp_bb__dp_listcomp_3_1(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, x = __dp__.take_args(_dp_args_ptr)
    _dp_tmp_1.append(x)
    return __dp__.jump(_dp_bb__dp_listcomp_3_3, (_dp_iter_2, _dp_tmp_1))


def _dp_bb__dp_listcomp_3_2(_dp_args_ptr):
    _dp_iter_2, _dp_tmp_1, _dp_tmp_3 = __dp__.take_args(_dp_args_ptr)
    x = _dp_tmp_3
    _dp_tmp_3 = None
    return __dp__.jump(_dp_bb__dp_listcomp_3_1, (_dp_iter_2, _dp_tmp_1, x))


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

    def _dp_bb__dp_class_ns_C_start(_dp_args_ptr):
        _dp_class_ns, _dp_classcell_arg = __dp__.take_args(_dp_args_ptr)
        _dp_classcell = _dp_classcell_arg
        __dp__.setitem(_dp_class_ns, "__module__", __name__)
        __dp__.setitem(_dp_class_ns, "__qualname__", "C")
        _dp_listcomp_3 = __dp__.def_fn(
            _dp_bb__dp_listcomp_3_start,
            "<listcomp>",
            "C._dp_listcomp_3",
            ("_dp_iter_2",),
            (("_dp_iter_2", None, __dp__.NO_DEFAULT),),
            __name__,
        )
        __dp__.setitem(
            _dp_class_ns,
            "xs",
            _dp_listcomp_3(__dp__.class_lookup_global(_dp_class_ns, "it", globals())),
        )
        return __dp__.ret(None)

    _dp_class_ns_C = __dp__.def_fn(
        _dp_bb__dp_class_ns_C_start,
        "_dp_class_ns_C",
        "_dp_class_ns_C",
        ("_dp_class_ns", "_dp_classcell_arg"),
        (
            ("_dp_class_ns", None, __dp__.NO_DEFAULT),
            ("_dp_classcell_arg", None, __dp__.NO_DEFAULT),
        ),
        __name__,
    )

    def _dp_bb__dp_define_class_C_start(_dp_args_ptr):
        return __dp__.ret(
            __dp__.create_class("C", _dp_class_ns_C, (), None, False, 3, ())
        )

    _dp_define_class_C = __dp__.def_fn(
        _dp_bb__dp_define_class_C_start,
        "_dp_define_class_C",
        "_dp_define_class_C",
        (),
        (),
        __name__,
    )
    __dp__.store_global(globals(), "C", _dp_define_class_C())
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# with_multi

with a as x, b as y:
    body()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_with_exit_3 = __dp__.contextmanager_get_exit(a)
    __dp__.store_global(globals(), "x", __dp__.contextmanager_enter(a))
    _dp_with_ok_4 = True
    try:
        _dp_with_exit_1 = __dp__.contextmanager_get_exit(b)
        __dp__.store_global(globals(), "y", __dp__.contextmanager_enter(b))
        _dp_with_ok_2 = True
        try:
            body()
        except:
            if __dp__.exception_matches(__dp__.current_exception(), BaseException):
                _dp_with_ok_2 = False
                __dp__.contextmanager_exit(_dp_with_exit_1, __dp__.exc_info())
            else:
                raise
        if _dp_with_ok_2:
            __dp__.contextmanager_exit(_dp_with_exit_1, None)
        _dp_with_exit_1 = None
    except:
        if __dp__.exception_matches(__dp__.current_exception(), BaseException):
            _dp_with_ok_4 = False
            __dp__.contextmanager_exit(_dp_with_exit_3, __dp__.exc_info())
        else:
            raise
    if _dp_with_ok_4:
        __dp__.contextmanager_exit(_dp_with_exit_3, None)
    _dp_with_exit_3 = None


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    _dp_with_exit_3 = None
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    _dp_with_exit_3 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.contextmanager_exit(_dp_with_exit_3, None)
    return __dp__.jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4 = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        _dp_with_ok_4,
        _dp_bb__dp_module_init_1,
        (_dp_with_exit_3,),
        _dp_bb__dp_module_init_0,
        (),
    )


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4 = __dp__.take_args(_dp_args_ptr)
    return __dp__.jump(_dp_bb__dp_module_init_2, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_4(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4 = __dp__.take_args(_dp_args_ptr)
    _dp_with_exit_1 = None
    return __dp__.jump(_dp_bb__dp_module_init_3, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_5(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1 = __dp__.take_args(_dp_args_ptr)
    __dp__.contextmanager_exit(_dp_with_exit_1, None)
    return __dp__.jump(_dp_bb__dp_module_init_4, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_6(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(
        _dp_args_ptr
    )
    return __dp__.brif(
        _dp_with_ok_2,
        _dp_bb__dp_module_init_5,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1),
        _dp_bb__dp_module_init_4,
        (_dp_with_exit_3, _dp_with_ok_4),
    )


def _dp_bb__dp_module_init_7(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(
        _dp_args_ptr
    )
    return __dp__.jump(
        _dp_bb__dp_module_init_6,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2),
    )


def _dp_bb__dp_module_init_8(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(
        _dp_args_ptr
    )
    body()
    return __dp__.jump(
        _dp_bb__dp_module_init_7,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2),
    )


def _dp_bb__dp_module_init_9(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_try_exc_11 = __dp__.DELETED
    return __dp__.jump(
        _dp_bb__dp_module_init_6,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2),
    )


def _dp_bb__dp_module_init_10(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1 = __dp__.take_args(_dp_args_ptr)
    _dp_with_ok_2 = False
    __dp__.contextmanager_exit(_dp_with_exit_1, __dp__.exc_info())
    return __dp__.jump(
        _dp_bb__dp_module_init_9,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2),
    )


def _dp_bb__dp_module_init_11(_dp_args_ptr):
    _dp_try_exc_11 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_11)


def _dp_bb__dp_module_init_12(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1 = __dp__.take_args(_dp_args_ptr)
    _dp_try_exc_11 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_11, BaseException),
        _dp_bb__dp_module_init_10,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1),
        _dp_bb__dp_module_init_11,
        (_dp_try_exc_11,),
    )


def _dp_bb__dp_module_init_13(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4 = __dp__.take_args(_dp_args_ptr)
    _dp_with_exit_1 = __dp__.contextmanager_get_exit(b)
    __dp__.store_global(globals(), "y", __dp__.contextmanager_enter(b))
    _dp_with_ok_2 = True
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_8,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1, _dp_with_ok_2),
        (_dp_bb__dp_module_init_7, _dp_bb__dp_module_init_8),
        _dp_bb__dp_module_init_12,
        (_dp_with_exit_3, _dp_with_ok_4, _dp_with_exit_1),
        (
            _dp_bb__dp_module_init_9,
            _dp_bb__dp_module_init_10,
            _dp_bb__dp_module_init_11,
            _dp_bb__dp_module_init_12,
        ),
        None,
        (),
        (),
        None,
    )


def _dp_bb__dp_module_init_14(_dp_args_ptr):
    _dp_with_exit_3, _dp_with_ok_4 = __dp__.take_args(_dp_args_ptr)
    _dp_try_exc_17 = __dp__.DELETED
    return __dp__.jump(_dp_bb__dp_module_init_2, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_15(_dp_args_ptr):
    _dp_with_exit_3 = __dp__.take_arg1(_dp_args_ptr)
    _dp_with_ok_4 = False
    __dp__.contextmanager_exit(_dp_with_exit_3, __dp__.exc_info())
    return __dp__.jump(_dp_bb__dp_module_init_14, (_dp_with_exit_3, _dp_with_ok_4))


def _dp_bb__dp_module_init_16(_dp_args_ptr):
    _dp_try_exc_17 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_17)


def _dp_bb__dp_module_init_17(_dp_args_ptr):
    _dp_with_exit_3 = __dp__.take_arg1(_dp_args_ptr)
    _dp_try_exc_17 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_17, BaseException),
        _dp_bb__dp_module_init_15,
        (_dp_with_exit_3,),
        _dp_bb__dp_module_init_16,
        (_dp_try_exc_17,),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_with_exit_3 = __dp__.contextmanager_get_exit(a)
    __dp__.store_global(globals(), "x", __dp__.contextmanager_enter(a))
    _dp_with_ok_4 = True
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_13,
        (_dp_with_exit_3, _dp_with_ok_4),
        (
            _dp_bb__dp_module_init_3,
            _dp_bb__dp_module_init_4,
            _dp_bb__dp_module_init_5,
            _dp_bb__dp_module_init_6,
            _dp_bb__dp_module_init_7,
            _dp_bb__dp_module_init_8,
            _dp_bb__dp_module_init_9,
            _dp_bb__dp_module_init_10,
            _dp_bb__dp_module_init_11,
            _dp_bb__dp_module_init_12,
            _dp_bb__dp_module_init_13,
        ),
        _dp_bb__dp_module_init_17,
        (_dp_with_exit_3,),
        (
            _dp_bb__dp_module_init_14,
            _dp_bb__dp_module_init_15,
            _dp_bb__dp_module_init_16,
            _dp_bb__dp_module_init_17,
        ),
        None,
        (),
        (),
        None,
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        _dp_iter_1 = __dp__.aiter(ait)
        while True:
            _dp_tmp_2 = await __dp__.anext_or_sentinel(_dp_iter_1)
            if __dp__.is_(_dp_tmp_2, __dp__.ITER_COMPLETE):
                break
            else:
                x = _dp_tmp_2
                _dp_tmp_2 = None
                body()


# -- bb --
async def _dp_bb_run_0(_dp_args_ptr):
    return __dp__.ret(None)


async def _dp_bb_run_1(_dp_args_ptr):
    _dp_tmp_2, _dp_iter_1 = __dp__.take_args(_dp_args_ptr)
    x = _dp_tmp_2
    _dp_tmp_2 = None
    body()
    return __dp__.jump(_dp_bb_run_3, (_dp_iter_1,))


async def _dp_bb_run_2(_dp_args_ptr):
    _dp_iter_1 = __dp__.take_arg1(_dp_args_ptr)
    _dp_tmp_2 = await __dp__.anext_or_sentinel(_dp_iter_1)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_2, __dp__.ITER_COMPLETE),
        _dp_bb_run_0,
        (),
        _dp_bb_run_1,
        (_dp_tmp_2, _dp_iter_1),
    )


async def _dp_bb_run_3(_dp_args_ptr):
    _dp_iter_1 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.jump(_dp_bb_run_2, (_dp_iter_1,))


async def _dp_bb_run_start(_dp_args_ptr):
    _dp_iter_1 = __dp__.aiter(ait)
    return __dp__.jump(_dp_bb_run_3, (_dp_iter_1,))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "run",
        __dp__.def_coro(_dp_bb_run_start, "run", "run", (), (), __name__),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        _dp_with_exit_1 = __dp__.asynccontextmanager_get_aexit(cm)
        x = await __dp__.asynccontextmanager_aenter(cm)
        _dp_with_ok_2 = True
        try:
            body()
        except:
            _dp_with_ok_2 = False
            _dp_with_suppress_3 = await __dp__.asynccontextmanager_aexit(
                _dp_with_exit_1, __dp__.exc_info()
            )
            if __dp__.not_(_dp_with_suppress_3):
                raise
        finally:
            if _dp_with_ok_2:
                await __dp__.asynccontextmanager_aexit(_dp_with_exit_1, None)
            _dp_with_exit_1 = None


# -- bb --
async def _dp_bb_run_0(_dp_args_ptr):
    _dp_with_exit_1 = None
    return __dp__.ret(None)


async def _dp_bb_run_1(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    await __dp__.asynccontextmanager_aexit(_dp_with_exit_1, None)
    return __dp__.jump(_dp_bb_run_0, ())


async def _dp_bb_run_2(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        _dp_with_ok_2, _dp_bb_run_1, (_dp_with_exit_1,), _dp_bb_run_0, ()
    )


async def _dp_bb_run_3(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    return __dp__.jump(_dp_bb_run_2, (_dp_with_exit_1, _dp_with_ok_2))


async def _dp_bb_run_4(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    body()
    return __dp__.jump(_dp_bb_run_3, (_dp_with_exit_1, _dp_with_ok_2))


async def _dp_bb_run_5(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.DELETED
    return __dp__.jump(_dp_bb_run_2, (_dp_with_exit_1, _dp_with_ok_2))


async def _dp_bb_run_6(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_7 = __dp__.take_args(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_7)


async def _dp_bb_run_7(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.current_exception()
    _dp_with_ok_2 = False
    _dp_with_suppress_3 = await __dp__.asynccontextmanager_aexit(
        _dp_with_exit_1, __dp__.exc_info()
    )
    return __dp__.brif(
        __dp__.not_(_dp_with_suppress_3),
        _dp_bb_run_6,
        (_dp_with_exit_1, _dp_with_ok_2, _dp_try_exc_7),
        _dp_bb_run_5,
        (_dp_with_exit_1, _dp_with_ok_2),
    )


async def _dp_bb_run_start(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.asynccontextmanager_get_aexit(cm)
    x = await __dp__.asynccontextmanager_aenter(cm)
    _dp_with_ok_2 = True
    return await __dp__.try_jump_term_async(
        _dp_bb_run_4,
        (_dp_with_exit_1, _dp_with_ok_2),
        (_dp_bb_run_3, _dp_bb_run_4),
        _dp_bb_run_7,
        (_dp_with_exit_1,),
        (_dp_bb_run_5, _dp_bb_run_6, _dp_bb_run_7),
        _dp_bb_run_2,
        (_dp_with_exit_1, _dp_with_ok_2),
        (_dp_bb_run_0, _dp_bb_run_1, _dp_bb_run_2),
        _dp_bb_run_8,
    )


async def _dp_bb_run_8(_dp_args_ptr):
    return __dp__.ret(None)


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "run",
        __dp__.def_coro(_dp_bb_run_start, "run", "run", (), (), __name__),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
    if __dp__.eq(_dp_match_1, 1):
        one()
    else:
        other()


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    one()
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    other()
    return __dp__.ret(None)


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_match_1 = value
    return __dp__.brif(
        __dp__.eq(_dp_match_1, 1),
        _dp_bb__dp_module_init_0,
        (),
        _dp_bb__dp_module_init_1,
        (),
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
def _dp_bb_gen_done(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb_gen_invalid(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    return __dp__.raise_(
        RuntimeError("invalid generator pc: {}".format(_dp_state["pc"]))
    )


def _dp_bb_gen_resume_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", 4)
    __dp__.setitem(_dp_state, "args", (_dp_state,))
    return __dp__.ret(1)


def _dp_bb_gen_resume_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "gen",
        __dp__.def_gen(
            2,
            (
                _dp_bb_gen_done,
                _dp_bb_gen_invalid,
                _dp_bb_gen_resume_0,
                _dp_bb_gen_resume_1,
            ),
            (-1, -1, -1, -1),
            "gen",
            "gen",
            (),
            (),
            __name__,
        ),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
def _dp_bb_gen_done(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb_gen_invalid(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    return __dp__.raise_(
        RuntimeError("invalid generator pc: {}".format(_dp_state["pc"]))
    )


def _dp_bb_gen_resume_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_iter_1 = iter(it)
    __dp__.setitem(_dp_state, "gi_yieldfrom", _dp_yield_from_iter_1)
    return __dp__.try_jump_term(
        _dp_bb_gen_internal_0,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1),
        (_dp_bb_gen_internal_0,),
        _dp_bb_gen_internal_1,
        (_dp_state, _dp_send_value, _dp_resume_exc),
        (_dp_bb_gen_internal_1, _dp_bb_gen_internal_2, _dp_bb_gen_internal_4),
        None,
        (),
        (),
        None,
    )


def _dp_bb_gen_internal_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_yield_from_y_2 = next(_dp_yield_from_iter_1)
    return __dp__.jump(
        _dp_bb_gen_internal_5,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_y_2,
        ),
    )


def _dp_bb_gen_internal_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_stop_4 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_yield_from_stop_4, StopIteration),
        _dp_bb_gen_internal_2,
        (_dp_state, _dp_send_value, _dp_resume_exc),
        _dp_bb_gen_internal_4,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_stop_4),
    )


def _dp_bb_gen_internal_2(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    return __dp__.jump(
        _dp_bb_gen_internal_3, (_dp_state, _dp_send_value, _dp_resume_exc)
    )


def _dp_bb_gen_internal_3(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "gi_yieldfrom", None)
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb_gen_internal_4(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_stop_4 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_yield_from_raise_6 = _dp_yield_from_stop_4
    return __dp__.jump(
        _dp_bb_gen_internal_10,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_raise_6),
    )


def _dp_bb_gen_internal_5(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_y_2,
    ) = __dp__.take_args(_dp_args_ptr)
    __dp__.setitem(_dp_state, "pc", 10)
    __dp__.setitem(_dp_state, "args", (_dp_state, _dp_yield_from_iter_1))
    return __dp__.ret(_dp_yield_from_y_2)


def _dp_bb_gen_resume_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_yield_from_sent_3 = _dp_send_value
    _dp_yield_from_exc_5 = _dp_resume_exc
    _dp_resume_exc = None
    return __dp__.brif(
        _dp_yield_from_exc_5 is not None,
        _dp_bb_gen_internal_6,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_exc_5,
        ),
        _dp_bb_gen_internal_13,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_sent_3,
        ),
    )


def _dp_bb_gen_internal_6(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
    ) = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        __dp__.exception_matches(_dp_yield_from_exc_5, GeneratorExit),
        _dp_bb_gen_internal_7,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_exc_5,
        ),
        _dp_bb_gen_internal_11,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_exc_5,
        ),
    )


def _dp_bb_gen_internal_7(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
    ) = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_close_7 = getattr(_dp_yield_from_iter_1, "close", None)
    return __dp__.brif(
        _dp_yield_from_close_7 is not None,
        _dp_bb_gen_internal_8,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_exc_5,
            _dp_yield_from_close_7,
        ),
        _dp_bb_gen_internal_9,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_exc_5),
    )


def _dp_bb_gen_internal_8(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_exc_5,
        _dp_yield_from_close_7,
    ) = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_close_7()
    return __dp__.jump(
        _dp_bb_gen_internal_9,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_exc_5),
    )


def _dp_bb_gen_internal_9(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_exc_5 = __dp__.take_args(
        _dp_args_ptr
    )
    _dp_yield_from_raise_6 = _dp_yield_from_exc_5
    return __dp__.jump(
        _dp_bb_gen_internal_10,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_raise_6),
    )


def _dp_bb_gen_internal_10(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_raise_6 = (
        __dp__.take_args(_dp_args_ptr)
    )
    __dp__.setitem(_dp_state, "gi_yieldfrom", None)
    return __dp__.raise_(_dp_yield_from_raise_6)


def _dp_bb_gen_internal_11(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
    ) = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_throw_8 = getattr(_dp_yield_from_iter_1, "throw", None)
    return __dp__.brif(
        _dp_yield_from_throw_8 is None,
        _dp_bb_gen_internal_9,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_exc_5),
        _dp_bb_gen_resume_2,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_exc_5,
            _dp_yield_from_throw_8,
        ),
    )


def _dp_bb_gen_resume_2(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
        _dp_yield_from_throw_8,
    ) = __dp__.take_args(_dp_args_ptr)
    return __dp__.try_jump_term(
        _dp_bb_gen_internal_12,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_exc_5,
            _dp_yield_from_throw_8,
        ),
        (_dp_bb_gen_internal_12,),
        _dp_bb_gen_internal_1,
        (_dp_state, _dp_send_value, _dp_resume_exc),
        (_dp_bb_gen_internal_1, _dp_bb_gen_internal_2, _dp_bb_gen_internal_4),
        None,
        (),
        (),
        None,
    )


def _dp_bb_gen_internal_12(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_exc_5,
        _dp_yield_from_throw_8,
    ) = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_y_2 = _dp_yield_from_throw_8(_dp_yield_from_exc_5)
    return __dp__.jump(
        _dp_bb_gen_internal_5,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_y_2,
        ),
    )


def _dp_bb_gen_internal_13(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_sent_3,
    ) = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        _dp_yield_from_sent_3 is None,
        _dp_bb_gen_internal_0,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_yield_from_iter_1),
        _dp_bb_gen_internal_14,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_sent_3,
        ),
    )


def _dp_bb_gen_internal_14(_dp_args_ptr):
    (
        _dp_state,
        _dp_send_value,
        _dp_resume_exc,
        _dp_yield_from_iter_1,
        _dp_yield_from_sent_3,
    ) = __dp__.take_args(_dp_args_ptr)
    _dp_yield_from_y_2 = _dp_yield_from_iter_1.send(_dp_yield_from_sent_3)
    return __dp__.jump(
        _dp_bb_gen_internal_5,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_yield_from_iter_1,
            _dp_yield_from_y_2,
        ),
    )


def _dp_bb_gen_resume_3(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc = __dp__.take_args(_dp_args_ptr)
    return __dp__.jump(_dp_bb_gen_resume_0, (_dp_state, _dp_send_value, _dp_resume_exc))


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "gen",
        __dp__.def_gen(
            20,
            (
                _dp_bb_gen_done,
                _dp_bb_gen_invalid,
                _dp_bb_gen_resume_0,
                _dp_bb_gen_internal_0,
                _dp_bb_gen_internal_1,
                _dp_bb_gen_internal_2,
                _dp_bb_gen_internal_3,
                _dp_bb_gen_internal_4,
                _dp_bb_gen_internal_5,
                _dp_bb_gen_resume_1,
                _dp_bb_gen_internal_6,
                _dp_bb_gen_internal_7,
                _dp_bb_gen_internal_8,
                _dp_bb_gen_internal_9,
                _dp_bb_gen_internal_10,
                _dp_bb_gen_internal_11,
                _dp_bb_gen_resume_2,
                _dp_bb_gen_internal_12,
                _dp_bb_gen_internal_13,
                _dp_bb_gen_internal_14,
                _dp_bb_gen_resume_3,
            ),
            (
                -1,
                -1,
                -1,
                2,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                -1,
                16,
                -1,
                -1,
                -1,
            ),
            "gen",
            "gen",
            (),
            (),
            __name__,
        ),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start

# with_exit_suppresses_exception

with Suppress():
    raise RuntimeError("boom")

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_3 = Suppress()
    _dp_with_exit_1 = __dp__.contextmanager_get_exit(_dp_tmp_3)
    __dp__.contextmanager_enter(_dp_tmp_3)
    _dp_with_ok_2 = True
    try:
        raise RuntimeError("boom")
    except:
        if __dp__.exception_matches(__dp__.current_exception(), BaseException):
            _dp_with_ok_2 = False
            __dp__.contextmanager_exit(_dp_with_exit_1, __dp__.exc_info())
        else:
            raise
    if _dp_with_ok_2:
        __dp__.contextmanager_exit(_dp_with_exit_1, None)
    _dp_with_exit_1 = None
    _dp_tmp_3 = None


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    _dp_with_exit_1 = None
    _dp_tmp_3 = None
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    __dp__.contextmanager_exit(_dp_with_exit_1, None)
    return __dp__.jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    return __dp__.brif(
        _dp_with_ok_2,
        _dp_bb__dp_module_init_1,
        (_dp_with_exit_1,),
        _dp_bb__dp_module_init_0,
        (),
    )


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    return __dp__.raise_(RuntimeError("boom"))


def _dp_bb__dp_module_init_4(_dp_args_ptr):
    _dp_with_exit_1, _dp_with_ok_2 = __dp__.take_args(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.DELETED
    return __dp__.jump(_dp_bb__dp_module_init_2, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_5(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    _dp_with_ok_2 = False
    __dp__.contextmanager_exit(_dp_with_exit_1, __dp__.exc_info())
    return __dp__.jump(_dp_bb__dp_module_init_4, (_dp_with_exit_1, _dp_with_ok_2))


def _dp_bb__dp_module_init_6(_dp_args_ptr):
    _dp_try_exc_7 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_7)


def _dp_bb__dp_module_init_7(_dp_args_ptr):
    _dp_with_exit_1 = __dp__.take_arg1(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_7, BaseException),
        _dp_bb__dp_module_init_5,
        (_dp_with_exit_1,),
        _dp_bb__dp_module_init_6,
        (_dp_try_exc_7,),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    _dp_tmp_3 = Suppress()
    _dp_with_exit_1 = __dp__.contextmanager_get_exit(_dp_tmp_3)
    __dp__.contextmanager_enter(_dp_tmp_3)
    _dp_with_ok_2 = True
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_3,
        (),
        (_dp_bb__dp_module_init_3,),
        _dp_bb__dp_module_init_7,
        (_dp_with_exit_1,),
        (
            _dp_bb__dp_module_init_4,
            _dp_bb__dp_module_init_5,
            _dp_bb__dp_module_init_6,
            _dp_bb__dp_module_init_7,
        ),
        None,
        (),
        (),
        None,
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        _dp_cell_x = __dp__.make_cell()
        __dp__.store_cell(_dp_cell_x, 5)

        def inner():
            return __dp__.load_cell(_dp_cell_x)

        return inner()


# -- bb --
def _dp_bb_inner_start(_dp_args_ptr):
    _dp_cell_x = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(__dp__.load_cell(_dp_cell_x))


def _dp_bb_outer_start(_dp_args_ptr):
    _dp_cell_x = __dp__.make_cell()
    __dp__.store_cell(_dp_cell_x, 5)
    inner = __dp__.def_fn(
        _dp_bb_inner_start,
        "inner",
        "outer.<locals>.inner",
        (("_dp_cell_x", _dp_cell_x),),
        (),
        __name__,
    )
    return __dp__.ret(inner())


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "outer",
        __dp__.def_fn(_dp_bb_outer_start, "outer", "outer", (), (), __name__),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        total = __dp__.add(a, b)
        if __dp__.gt(total, 5):
            return a
        else:
            return b


# -- bb --
def _dp_bb_choose_0(_dp_args_ptr):
    a = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(a)


def _dp_bb_choose_1(_dp_args_ptr):
    b = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.ret(b)


def _dp_bb_choose_start(_dp_args_ptr):
    a, b = __dp__.take_args(_dp_args_ptr)
    total = __dp__.add(a, b)
    return __dp__.brif(
        __dp__.gt(total, 5), _dp_bb_choose_0, (a,), _dp_bb_choose_1, (b,)
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "choose",
        __dp__.def_fn(
            _dp_bb_choose_start,
            "choose",
            "choose",
            ("a", "b"),
            (("a", None, __dp__.NO_DEFAULT), ("b", None, __dp__.NO_DEFAULT)),
            __name__,
        ),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        _dp_cell_x = __dp__.make_cell()
        __dp__.store_cell(_dp_cell_x, 5)

        def inner():
            nonlocal _dp_cell_x
            __dp__.store_cell(_dp_cell_x, 2)
            return __dp__.load_cell(_dp_cell_x)

        return inner()


# -- bb --
def _dp_bb_inner_start(_dp_args_ptr):
    _dp_cell_x = __dp__.take_arg1(_dp_args_ptr)
    __dp__.store_cell(_dp_cell_x, 2)
    return __dp__.ret(__dp__.load_cell(_dp_cell_x))


def _dp_bb_outer_start(_dp_args_ptr):
    _dp_cell_x = __dp__.make_cell()
    __dp__.store_cell(_dp_cell_x, 5)
    inner = __dp__.def_fn(
        _dp_bb_inner_start,
        "inner",
        "outer.<locals>.inner",
        (("_dp_cell_x", _dp_cell_x),),
        (),
        __name__,
    )
    return __dp__.ret(inner())


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "outer",
        __dp__.def_fn(_dp_bb_outer_start, "outer", "outer", (), (), __name__),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        if __dp__.exception_matches(__dp__.current_exception(), Exception):
            print(2)
        else:
            raise


# -- bb --
def _dp_bb__dp_module_init_0(_dp_args_ptr):
    return __dp__.ret(None)


def _dp_bb__dp_module_init_1(_dp_args_ptr):
    print(1)
    return __dp__.jump(_dp_bb__dp_module_init_0, ())


def _dp_bb__dp_module_init_2(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.DELETED
    return __dp__.ret(None)


def _dp_bb__dp_module_init_3(_dp_args_ptr):
    print(2)
    return __dp__.jump(_dp_bb__dp_module_init_2, ())


def _dp_bb__dp_module_init_4(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.take_arg1(_dp_args_ptr)
    return __dp__.raise_(_dp_try_exc_4)


def _dp_bb__dp_module_init_5(_dp_args_ptr):
    _dp_try_exc_4 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_4, Exception),
        _dp_bb__dp_module_init_3,
        (),
        _dp_bb__dp_module_init_4,
        (_dp_try_exc_4,),
    )


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    return __dp__.try_jump_term(
        _dp_bb__dp_module_init_1,
        (),
        (_dp_bb__dp_module_init_0, _dp_bb__dp_module_init_1),
        _dp_bb__dp_module_init_5,
        (),
        (
            _dp_bb__dp_module_init_2,
            _dp_bb__dp_module_init_3,
            _dp_bb__dp_module_init_4,
            _dp_bb__dp_module_init_5,
        ),
        None,
        (),
        (),
        None,
    )


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
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
        for i in a:
            try:
                j = __dp__.add(i, 1)
                yield j
            except:
                if __dp__.exception_matches(__dp__.current_exception(), Exception):
                    print("oops")
                else:
                    raise


# -- bb --
def _dp_bb_complicated_done(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = __dp__.take_args(
        _dp_args_ptr
    )
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb_complicated_invalid(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = __dp__.take_args(
        _dp_args_ptr
    )
    return __dp__.raise_(
        RuntimeError("invalid generator pc: {}".format(_dp_state["pc"]))
    )


def _dp_bb_complicated_resume_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    return __dp__.jump(
        _dp_bb_complicated_internal_6,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
    )


def _dp_bb_complicated_internal_0(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, i, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    j = __dp__.add(i, 1)
    __dp__.setitem(_dp_state, "pc", 3)
    __dp__.setitem(_dp_state, "args", (_dp_state, _dp_iter_1, _dp_try_exc_7))
    return __dp__.ret(j)


def _dp_bb_complicated_internal_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    _dp_try_exc_7 = __dp__.DELETED
    return __dp__.jump(
        _dp_bb_complicated_internal_6,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
    )


def _dp_bb_complicated_internal_2(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    print("oops")
    return __dp__.jump(
        _dp_bb_complicated_internal_1,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
    )


def _dp_bb_complicated_internal_3(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = __dp__.take_args(
        _dp_args_ptr
    )
    return __dp__.raise_(_dp_try_exc_7)


def _dp_bb_complicated_internal_4(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    _dp_try_exc_7 = __dp__.current_exception()
    return __dp__.brif(
        __dp__.exception_matches(_dp_try_exc_7, Exception),
        _dp_bb_complicated_internal_2,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
        _dp_bb_complicated_internal_3,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_try_exc_7),
    )


def _dp_bb_complicated_resume_1(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, i, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    return __dp__.try_jump_term(
        _dp_bb_complicated_internal_0,
        (_dp_state, _dp_send_value, _dp_resume_exc, i, _dp_iter_1, _dp_try_exc_7),
        (_dp_bb_complicated_resume_0, _dp_bb_complicated_internal_0),
        _dp_bb_complicated_internal_4,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
        (
            _dp_bb_complicated_internal_1,
            _dp_bb_complicated_internal_2,
            _dp_bb_complicated_internal_3,
            _dp_bb_complicated_internal_4,
        ),
        None,
        (),
        (),
        None,
    )


def _dp_bb_complicated_internal_5(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_tmp_2, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    i = _dp_tmp_2
    _dp_tmp_2 = None
    return __dp__.jump(
        _dp_bb_complicated_resume_1,
        (_dp_state, _dp_send_value, _dp_resume_exc, i, _dp_iter_1, _dp_try_exc_7),
    )


def _dp_bb_complicated_internal_6(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7 = (
        __dp__.take_args(_dp_args_ptr)
    )
    _dp_tmp_2 = __dp__.next_or_sentinel(_dp_iter_1)
    return __dp__.brif(
        __dp__.is_(_dp_tmp_2, __dp__.ITER_COMPLETE),
        _dp_bb_complicated_internal_7,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_try_exc_7),
        _dp_bb_complicated_internal_5,
        (
            _dp_state,
            _dp_send_value,
            _dp_resume_exc,
            _dp_tmp_2,
            _dp_iter_1,
            _dp_try_exc_7,
        ),
    )


def _dp_bb_complicated_resume_2(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, a = __dp__.take_args(_dp_args_ptr)
    _dp_try_exc_7 = __dp__.DELETED
    _dp_iter_1 = __dp__.iter(a)
    return __dp__.jump(
        _dp_bb_complicated_internal_6,
        (_dp_state, _dp_send_value, _dp_resume_exc, _dp_iter_1, _dp_try_exc_7),
    )


def _dp_bb_complicated_internal_7(_dp_args_ptr):
    _dp_state, _dp_send_value, _dp_resume_exc, _dp_try_exc_7 = __dp__.take_args(
        _dp_args_ptr
    )
    __dp__.setitem(_dp_state, "pc", __dp__._GEN_PC_DONE)
    return __dp__.ret(None)


def _dp_bb__dp_module_init_start(_dp_args_ptr):
    __dp__.store_global(
        globals(),
        "complicated",
        __dp__.def_gen(
            11,
            (
                _dp_bb_complicated_done,
                _dp_bb_complicated_invalid,
                _dp_bb_complicated_resume_0,
                _dp_bb_complicated_internal_0,
                _dp_bb_complicated_internal_1,
                _dp_bb_complicated_internal_2,
                _dp_bb_complicated_internal_3,
                _dp_bb_complicated_internal_4,
                _dp_bb_complicated_resume_1,
                _dp_bb_complicated_internal_5,
                _dp_bb_complicated_internal_6,
                _dp_bb_complicated_resume_2,
                _dp_bb_complicated_internal_7,
            ),
            (-1, -1, 8, 8, -1, -1, -1, -1, -1, -1, -1, -1, -1),
            "complicated",
            "complicated",
            ("a",),
            (("a", None, __dp__.NO_DEFAULT),),
            __name__,
        ),
    )
    return __dp__.ret(None)


_dp_module_init = __dp__.def_fn(
    _dp_bb__dp_module_init_start, "_dp_module_init", "_dp_module_init", (), (), __name__
)
del _dp_bb__dp_module_init_start
