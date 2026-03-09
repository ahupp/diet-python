# class_with_method


class C:
    x: int = 1

    def m(self):
        return self.x


# ==


# class_method_named_open_calls_builtin


class Wrapper:
    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        return open(mode, encoding=encoding)


# ==


# class_with_base


class D(Base):
    pass


# ==


# class_scope_inner_capture


def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


# ==


# class_super_empty_classcell


class X:
    def f(x):
        nonlocal __class__
        del __class__
        super()


# ==


# nested classes


class A:
    class B:
        pass


# ==


# nested classes with weird scoping


def foo():
    class A:
        global B

        class B:
            pass


# ==
