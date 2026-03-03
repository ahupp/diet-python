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
