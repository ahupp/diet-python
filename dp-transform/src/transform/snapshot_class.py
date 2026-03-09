# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


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

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


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

# class_with_base


class D(Base):
    pass


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

# class_scope_inner_capture


def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


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

# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


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

# nested classes


class A:
    class B:
        pass


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

# nested classes with weird scoping


def foo():
    class A:
        global B

        class B:
            pass


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
