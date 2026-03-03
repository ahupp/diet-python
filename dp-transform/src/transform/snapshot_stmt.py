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

# assign_attr

obj.x = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), 1)


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

# assign_subscript

obj[i] = v

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setitem(obj, i, v)


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

# assign_multi_targets

a = b = f()

# ==


# -- pre-bb --
def _dp_module_init():
    _dp_tmp_1 = f()
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"a"), _dp_tmp_1)
    __dp_store_global(globals(), __dp_decode_literal_bytes(b"b"), _dp_tmp_1)


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

# ann_assign_attr

obj.x: int = 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), 1)


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

# aug_assign_attr

obj.x += 1

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_setattr(obj, __dp_decode_literal_bytes(b"x"), __dp_iadd(obj.x, 1))


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

# delete_mixed

del obj.x, obj[i], x

# ==


# -- pre-bb --
def _dp_module_init():
    __dp_delattr(obj, __dp_decode_literal_bytes(b"x"))
    __dp_delitem(obj, i)
    __dp_delitem(globals(), __dp_decode_literal_bytes(b"x"))


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

# assert_no_msg

assert cond

# ==


# -- pre-bb --
def _dp_module_init():
    if __debug__:
        if __dp_not_(cond):
            raise __dp_AssertionError


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

# assert_with_msg

assert cond, "oops"

# ==


# -- pre-bb --
def _dp_module_init():
    if __debug__:
        if __dp_not_(cond):
            raise __dp_AssertionError(__dp_decode_literal_bytes(b"oops"))


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

# raise_from

raise E from cause

# ==


# -- pre-bb --
def _dp_module_init():
    raise __dp_raise_from(E, cause)


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

# generator_yield


def gen():
    yield 1


# ==


# -- pre-bb --
def _dp_module_init():

    def gen():
        yield 1


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

# yield_from


def gen():
    yield from it


# ==


# -- pre-bb --
def _dp_module_init():

    def gen():
        yield from it


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
