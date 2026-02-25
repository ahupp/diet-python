# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


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
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"x"), 1)

        def m(self):
            return self.x

        def __annotate_func__(_dp_format, _dp=__dp__):
            if _dp.eq(_dp_format, 4):
                return _dp.dict(
                    (
                        (
                            __dp_decode_literal_bytes(b"x"),
                            __dp_decode_literal_bytes(b"int"),
                        ),
                    )
                )
            if _dp.gt(_dp_format, 2):
                raise _dp.builtins.NotImplementedError
            return _dp.dict(((__dp_decode_literal_bytes(b"x"), int),))

    def _dp_define_class_C(_dp_class_ns_fn):
        return __dp_create_class(
            __dp_decode_literal_bytes(b"C"), _dp_class_ns_fn, (), None, False, 3, ()
        )

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"C"), _dp_define_class_C(_dp_class_ns_C)
    )


# -- bb --
def _dp_bb__dp_module_init_start():

    def _dp_bb_m_start(self):
        self = self.take()
        return __dp_ret(__dp_getattr(self, __dp_decode_literal_bytes(b"x")))

    def _dp_bb__dp_class_ns_C_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"C"),
        )
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"x"), 1)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"m"),
            __dp_def_fn(
                _dp_bb_m_start,
                __dp_decode_literal_bytes(b"m"),
                __dp_decode_literal_bytes(b"C.m"),
                ("self",),
                (("self", None, __dp__.NO_DEFAULT),),
                __dp_globals(),
                __name__,
                __dp_NONE,
                __dp_NONE,
            ),
        )
        __annotate_func__ = __dp_exec_function_def_source(
            __dp_decode_literal_bytes(
                b'def __annotate_func__(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_decode_literal_bytes=__dp_decode_literal_bytes):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict(((__dp_decode_literal_bytes(b"x"), __dp_decode_literal_bytes(b"int")),))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict(((__dp_decode_literal_bytes(b"x"), int),))'
            ),
            __dp_globals(),
            (),
            __dp_decode_literal_bytes(b"__annotate_func__"),
        )
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__annotate_func__"),
            __dp_update_fn(
                __annotate_func__,
                __dp_decode_literal_bytes(b"C.__annotate_func__"),
                __dp_decode_literal_bytes(b"__annotate_func__"),
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

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_Wrapper(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"Wrapper"),
        )

        def open(
            self,
            mode: __dp_class_lookup_global(
                _dp_class_ns, __dp_decode_literal_bytes(b"str"), globals()
            ) = __dp_decode_literal_bytes(b"r"),
            *,
            encoding: __dp_class_lookup_global(
                _dp_class_ns, __dp_decode_literal_bytes(b"str"), globals()
            ) = __dp_decode_literal_bytes(b"utf8"),
        ):
            return open(mode, encoding=encoding)

    def _dp_define_class_Wrapper(_dp_class_ns_fn):
        return __dp_create_class(
            __dp_decode_literal_bytes(b"Wrapper"),
            _dp_class_ns_fn,
            (),
            None,
            False,
            3,
            (),
        )

    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"Wrapper"),
        _dp_define_class_Wrapper(_dp_class_ns_Wrapper),
    )


# -- bb --
def _dp_bb__dp_module_init_start():

    def _dp_bb_open_start(mode, encoding):
        mode, encoding = mode.take(), encoding.take()
        return __dp_ret(open(mode, encoding=encoding))

    def _dp_bb__dp_class_ns_Wrapper_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"Wrapper"),
        )
        _dp_fn___annotate___open = __dp_exec_function_def_source(
            __dp_decode_literal_bytes(
                b'def _dp_fn___annotate___open(_dp_format, _dp=__dp__, *, __dp__=__dp__, __dp_class_lookup_global=__dp_class_lookup_global, __dp_decode_literal_bytes=__dp_decode_literal_bytes, _dp_class_ns=_dp_class_ns):\n    if _dp.eq(_dp_format, 4):\n        return _dp.dict((("mode", \'__dp_class_lookup_global(_dp_class_ns, __dp_decode_literal_bytes(b"str"), globals())\'), ("encoding", \'__dp_class_lookup_global(_dp_class_ns, __dp_decode_literal_bytes(b"str"), globals())\')))\n    if _dp.gt(_dp_format, 2):\n        raise _dp.builtins.NotImplementedError\n    return _dp.dict((("mode", __dp_class_lookup_global(_dp_class_ns, __dp_decode_literal_bytes(b"str"), globals())), ("encoding", __dp_class_lookup_global(_dp_class_ns, __dp_decode_literal_bytes(b"str"), globals()))))'
            ),
            __dp_globals(),
            (("_dp_class_ns", _dp_class_ns),),
            __dp_decode_literal_bytes(b"_dp_fn___annotate___open"),
        )
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"open"),
            __dp_def_fn(
                _dp_bb_open_start,
                __dp_decode_literal_bytes(b"open"),
                __dp_decode_literal_bytes(b"Wrapper.open"),
                ("mode", "encoding"),
                (
                    ("self", None, __dp__.NO_DEFAULT),
                    ("mode", None, __dp_decode_literal_bytes(b"r")),
                    ("kw:encoding", None, __dp_decode_literal_bytes(b"utf8")),
                ),
                __dp_globals(),
                __name__,
                __dp_NONE,
                _dp_fn___annotate___open,
            ),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_Wrapper_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"Wrapper"),
                _dp_class_ns_fn,
                (),
                __dp_NONE,
                __dp_FALSE,
                3,
                (),
            )
        )

    _dp_class_ns_Wrapper = __dp_def_fn(
        _dp_bb__dp_class_ns_Wrapper_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_Wrapper"),
        __dp_decode_literal_bytes(b"_dp_class_ns_Wrapper"),
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
    _dp_define_class_Wrapper = __dp_def_fn(
        _dp_bb__dp_define_class_Wrapper_start,
        __dp_decode_literal_bytes(b"_dp_define_class_Wrapper"),
        __dp_decode_literal_bytes(b"_dp_define_class_Wrapper"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"Wrapper"),
        _dp_define_class_Wrapper(_dp_class_ns_Wrapper),
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

# class_with_base


class D(Base):
    pass


# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_D(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"D"),
        )

    def _dp_define_class_D(_dp_class_ns_fn):
        return __dp_create_class(
            __dp_decode_literal_bytes(b"D"),
            _dp_class_ns_fn,
            (Base,),
            None,
            False,
            3,
            (),
        )

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"D"), _dp_define_class_D(_dp_class_ns_D)
    )


# -- bb --
def _dp_bb__dp_module_init_start():

    def _dp_bb__dp_class_ns_D_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"D"),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_D_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"D"),
                _dp_class_ns_fn,
                (Base,),
                __dp_NONE,
                __dp_FALSE,
                3,
                (),
            )
        )

    _dp_class_ns_D = __dp_def_fn(
        _dp_bb__dp_class_ns_D_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_D"),
        __dp_decode_literal_bytes(b"_dp_class_ns_D"),
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
    _dp_define_class_D = __dp_def_fn(
        _dp_bb__dp_define_class_D_start,
        __dp_decode_literal_bytes(b"_dp_define_class_D"),
        __dp_decode_literal_bytes(b"_dp_define_class_D"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"D"), _dp_define_class_D(_dp_class_ns_D)
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

# class_scope_inner_capture


def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


# ==


# -- pre-bb --
def _dp_module_init():

    def outer():
        _dp_cell_x = __dp_make_cell()
        __dp_store_cell(_dp_cell_x, __dp_decode_literal_bytes(b"outer"))

        def _dp_class_ns_Inner(_dp_class_ns, _dp_classcell_arg):
            _dp_classcell = _dp_classcell_arg
            __dp_setitem(
                _dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__
            )
            __dp_setitem(
                _dp_class_ns,
                __dp_decode_literal_bytes(b"__qualname__"),
                __dp_decode_literal_bytes(b"outer.<locals>.Inner"),
            )
            __dp_setitem(
                _dp_class_ns,
                __dp_decode_literal_bytes(b"y"),
                __dp_class_lookup_cell(
                    _dp_class_ns, __dp_decode_literal_bytes(b"x"), _dp_cell_x
                ),
            )

        def _dp_define_class_Inner(_dp_class_ns_fn):
            return __dp_create_class(
                __dp_decode_literal_bytes(b"Inner"),
                _dp_class_ns_fn,
                (),
                None,
                False,
                6,
                (),
            )

        Inner = _dp_define_class_Inner(_dp_class_ns_Inner)
        return Inner.y


# -- bb --
def _dp_bb_outer_start():

    def _dp_bb__dp_class_ns_Inner_start(_dp_class_ns, _dp_classcell_arg, _dp_cell_x):
        _dp_class_ns, _dp_classcell_arg, _dp_cell_x = (
            _dp_class_ns.take(),
            _dp_classcell_arg.take(),
            _dp_cell_x.take(),
        )
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"outer.<locals>.Inner"),
        )
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"y"),
            __dp_class_lookup_cell(
                _dp_class_ns, __dp_decode_literal_bytes(b"x"), _dp_cell_x
            ),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_Inner_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"Inner"),
                _dp_class_ns_fn,
                (),
                __dp_NONE,
                __dp_FALSE,
                6,
                (),
            )
        )

    _dp_cell_x = __dp_make_cell()
    __dp_store_cell(_dp_cell_x, __dp_decode_literal_bytes(b"outer"))
    _dp_class_ns_Inner = __dp_def_fn(
        _dp_bb__dp_class_ns_Inner_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_Inner"),
        __dp_decode_literal_bytes(b"_dp_class_ns_Inner"),
        ("_dp_class_ns", "_dp_classcell_arg", ("_dp_cell_x", _dp_cell_x)),
        (
            ("_dp_class_ns", None, __dp__.NO_DEFAULT),
            ("_dp_classcell_arg", None, __dp__.NO_DEFAULT),
        ),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    _dp_define_class_Inner = __dp_def_fn(
        _dp_bb__dp_define_class_Inner_start,
        __dp_decode_literal_bytes(b"_dp_define_class_Inner"),
        __dp_decode_literal_bytes(b"_dp_define_class_Inner"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    Inner = _dp_define_class_Inner(_dp_class_ns_Inner)
    return __dp_ret(__dp_getattr(Inner, __dp_decode_literal_bytes(b"y")))


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

# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_X(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"X"),
        )

        def f(x):
            __dp_delattr(_dp_classcell, __dp_decode_literal_bytes(b"cell_contents"))
            __dp_call_super(super, _dp_classcell, x)

    def _dp_define_class_X(_dp_class_ns_fn):
        return __dp_create_class(
            __dp_decode_literal_bytes(b"X"), _dp_class_ns_fn, (), None, True, 3, ()
        )

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"X"), _dp_define_class_X(_dp_class_ns_X)
    )


# -- bb --
def _dp_bb__dp_module_init_start():

    def _dp_bb_f_start(x, _dp_classcell):
        x, _dp_classcell = x.take(), _dp_classcell.take()
        __dp_delattr(_dp_classcell, __dp_decode_literal_bytes(b"cell_contents"))
        __dp_call_super(super, _dp_classcell, x)
        return __dp_ret(None)

    def _dp_bb__dp_class_ns_X_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"X"),
        )
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"f"),
            __dp_def_fn(
                _dp_bb_f_start,
                __dp_decode_literal_bytes(b"f"),
                __dp_decode_literal_bytes(b"X.f"),
                ("x", ("_dp_classcell", _dp_classcell)),
                (("x", None, __dp__.NO_DEFAULT),),
                __dp_globals(),
                __name__,
                __dp_NONE,
                __dp_NONE,
            ),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_X_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"X"),
                _dp_class_ns_fn,
                (),
                __dp_NONE,
                __dp_TRUE,
                3,
                (),
            )
        )

    _dp_class_ns_X = __dp_def_fn(
        _dp_bb__dp_class_ns_X_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_X"),
        __dp_decode_literal_bytes(b"_dp_class_ns_X"),
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
    _dp_define_class_X = __dp_def_fn(
        _dp_bb__dp_define_class_X_start,
        __dp_decode_literal_bytes(b"_dp_define_class_X"),
        __dp_decode_literal_bytes(b"_dp_define_class_X"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"X"), _dp_define_class_X(_dp_class_ns_X)
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

# nested classes


class A:
    class B:
        pass


# ==


# -- pre-bb --
def _dp_module_init():

    def _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"A.B"),
        )

    def _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"A"),
        )

        def _dp_define_class_B(_dp_class_ns_fn):
            return __dp_create_class(
                __dp_decode_literal_bytes(b"B"), _dp_class_ns_fn, (), None, False, 4, ()
            )

        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"B"),
            _dp_define_class_B(_dp_class_ns_B),
        )

    def _dp_define_class_A(_dp_class_ns_fn):
        return __dp_create_class(
            __dp_decode_literal_bytes(b"A"), _dp_class_ns_fn, (), None, False, 3, ()
        )

    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"A"), _dp_define_class_A(_dp_class_ns_A)
    )


# -- bb --
def _dp_bb__dp_module_init_start():

    def _dp_bb__dp_class_ns_B_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"A.B"),
        )
        return __dp_ret(None)

    def _dp_bb__dp_class_ns_A_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()

        def _dp_bb__dp_define_class_B_start(_dp_class_ns_fn):
            _dp_class_ns_fn = _dp_class_ns_fn.take()
            return __dp_ret(
                __dp_create_class(
                    __dp_decode_literal_bytes(b"B"),
                    _dp_class_ns_fn,
                    (),
                    __dp_NONE,
                    __dp_FALSE,
                    4,
                    (),
                )
            )

        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"A"),
        )
        _dp_define_class_B = __dp_def_fn(
            _dp_bb__dp_define_class_B_start,
            __dp_decode_literal_bytes(b"_dp_define_class_B"),
            __dp_decode_literal_bytes(b"_dp_define_class_B"),
            ("_dp_class_ns_fn",),
            (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
        )
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"B"),
            _dp_define_class_B(_dp_class_ns_B),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_A_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"A"),
                _dp_class_ns_fn,
                (),
                __dp_NONE,
                __dp_FALSE,
                3,
                (),
            )
        )

    _dp_class_ns_B = __dp_def_fn(
        _dp_bb__dp_class_ns_B_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_B"),
        __dp_decode_literal_bytes(b"_dp_class_ns_B"),
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
    _dp_class_ns_A = __dp_def_fn(
        _dp_bb__dp_class_ns_A_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_A"),
        __dp_decode_literal_bytes(b"_dp_class_ns_A"),
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
    _dp_define_class_A = __dp_def_fn(
        _dp_bb__dp_define_class_A_start,
        __dp_decode_literal_bytes(b"_dp_define_class_A"),
        __dp_decode_literal_bytes(b"_dp_define_class_A"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    __dp_store_global(
        globals(), __dp_decode_literal_bytes(b"A"), _dp_define_class_A(_dp_class_ns_A)
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

# nested classes with weird scoping


def foo():
    class A:
        global B

        class B:
            pass


# ==


# -- pre-bb --
def _dp_module_init():

    def foo():

        def _dp_class_ns_B(_dp_class_ns, _dp_classcell_arg):
            _dp_classcell = _dp_classcell_arg
            __dp_setitem(
                _dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__
            )
            __dp_setitem(
                _dp_class_ns,
                __dp_decode_literal_bytes(b"__qualname__"),
                __dp_decode_literal_bytes(b"B"),
            )

        def _dp_class_ns_A(_dp_class_ns, _dp_classcell_arg):
            _dp_classcell = _dp_classcell_arg
            __dp_setitem(
                _dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__
            )
            __dp_setitem(
                _dp_class_ns,
                __dp_decode_literal_bytes(b"__qualname__"),
                __dp_decode_literal_bytes(b"foo.<locals>.A"),
            )

            def _dp_define_class_B(_dp_class_ns_fn):
                return __dp_create_class(
                    __dp_decode_literal_bytes(b"B"),
                    _dp_class_ns_fn,
                    (),
                    None,
                    False,
                    7,
                    (),
                )

            __dp_store_global(
                globals(),
                __dp_decode_literal_bytes(b"B"),
                _dp_define_class_B(_dp_class_ns_B),
            )

        def _dp_define_class_A(_dp_class_ns_fn):
            return __dp_create_class(
                __dp_decode_literal_bytes(b"A"), _dp_class_ns_fn, (), None, False, 4, ()
            )

        A = _dp_define_class_A(_dp_class_ns_A)


# -- bb --
def _dp_bb_foo_start():

    def _dp_bb__dp_class_ns_B_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()
        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"B"),
        )
        return __dp_ret(None)

    def _dp_bb__dp_class_ns_A_start(_dp_class_ns, _dp_classcell_arg):
        _dp_class_ns, _dp_classcell_arg = _dp_class_ns.take(), _dp_classcell_arg.take()

        def _dp_bb__dp_define_class_B_start(_dp_class_ns_fn):
            _dp_class_ns_fn = _dp_class_ns_fn.take()
            return __dp_ret(
                __dp_create_class(
                    __dp_decode_literal_bytes(b"B"),
                    _dp_class_ns_fn,
                    (),
                    __dp_NONE,
                    __dp_FALSE,
                    7,
                    (),
                )
            )

        _dp_classcell = _dp_classcell_arg
        __dp_setitem(_dp_class_ns, __dp_decode_literal_bytes(b"__module__"), __name__)
        __dp_setitem(
            _dp_class_ns,
            __dp_decode_literal_bytes(b"__qualname__"),
            __dp_decode_literal_bytes(b"foo.<locals>.A"),
        )
        _dp_define_class_B = __dp_def_fn(
            _dp_bb__dp_define_class_B_start,
            __dp_decode_literal_bytes(b"_dp_define_class_B"),
            __dp_decode_literal_bytes(b"_dp_define_class_B"),
            ("_dp_class_ns_fn",),
            (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
            __dp_globals(),
            __name__,
            __dp_NONE,
            __dp_NONE,
        )
        __dp_store_global(
            globals(),
            __dp_decode_literal_bytes(b"B"),
            _dp_define_class_B(_dp_class_ns_B),
        )
        return __dp_ret(None)

    def _dp_bb__dp_define_class_A_start(_dp_class_ns_fn):
        _dp_class_ns_fn = _dp_class_ns_fn.take()
        return __dp_ret(
            __dp_create_class(
                __dp_decode_literal_bytes(b"A"),
                _dp_class_ns_fn,
                (),
                __dp_NONE,
                __dp_FALSE,
                4,
                (),
            )
        )

    _dp_class_ns_B = __dp_def_fn(
        _dp_bb__dp_class_ns_B_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_B"),
        __dp_decode_literal_bytes(b"_dp_class_ns_B"),
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
    _dp_class_ns_A = __dp_def_fn(
        _dp_bb__dp_class_ns_A_start,
        __dp_decode_literal_bytes(b"_dp_class_ns_A"),
        __dp_decode_literal_bytes(b"_dp_class_ns_A"),
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
    _dp_define_class_A = __dp_def_fn(
        _dp_bb__dp_define_class_A_start,
        __dp_decode_literal_bytes(b"_dp_define_class_A"),
        __dp_decode_literal_bytes(b"_dp_define_class_A"),
        ("_dp_class_ns_fn",),
        (("_dp_class_ns_fn", None, __dp__.NO_DEFAULT),),
        __dp_globals(),
        __name__,
        __dp_NONE,
        __dp_NONE,
    )
    A = _dp_define_class_A(_dp_class_ns_A)
    return __dp_ret(None)


def _dp_bb__dp_module_init_start():
    __dp_store_global(
        globals(),
        __dp_decode_literal_bytes(b"foo"),
        __dp_def_fn(
            _dp_bb_foo_start,
            __dp_decode_literal_bytes(b"foo"),
            __dp_decode_literal_bytes(b"foo"),
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
