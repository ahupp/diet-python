# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# class_with_base


class D(Base):
    pass


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# class_scope_inner_capture


def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# nested classes


class A:
    class B:
        pass


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked

# nested classes with weird scoping


def foo():
    class A:
        global B

        class B:
            pass


# ==

# snapshot regeneration failed
# panic: ast-to-ast pass should be tracked
