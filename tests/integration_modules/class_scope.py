results = {}

x = "global"


class C1:
    x = "class"

    def read():
        return x


results["class_attr_vs_global"] = (C1.x, C1.read(), x)

x = "module"


class C2:
    global x
    x = "class-global"
    y = "class-attr"


results["class_global_assignment"] = (x, getattr(C2, "x", None), C2.y)

x = "module"


class C3:
    def set_x():
        global x
        x = "method-global"

    def read_x():
        return x


C3.set_x()
results["class_method_global_assignment"] = (x, C3.read_x())


class C4:
    def outer():
        x = "outer"

        def inner():
            nonlocal x
            x = "inner"

        inner()
        return x


results["class_method_nonlocal_inner"] = C4.outer()


def outer_with_inner_class():
    x = "outer"

    class Inner:
        y = x

    return Inner.y


results["def_with_inner_class_capture"] = outer_with_inner_class()

x = "module"


def outer_with_inner_class_global_assignment():
    x = "outer"

    class Inner:
        global x
        x = "class-global"
        y = "class-attr"

    return (x, getattr(Inner, "x", None), Inner.y)


results["def_with_inner_class_global_assignment"] = (
    outer_with_inner_class_global_assignment(),
    x,
)

x = "module"


def outer_with_class_method_reads_global():
    x = "outer"

    class Inner:
        x = "class"

        def read():
            return x

    return (Inner.x, Inner.read(), x)


results["def_with_class_with_method_reads_global"] = outer_with_class_method_reads_global()


def outer_with_nonlocal_and_inner_class():
    x = "outer"

    def inner():
        nonlocal x
        x = "inner"

        class Inner:
            y = x

        return Inner.y

    y = inner()
    return (x, y)


results["def_with_nonlocal_and_inner_class"] = outer_with_nonlocal_and_inner_class()


def nonlocal_in_class_body_error():
    try:
        exec("class Bad:\\n    nonlocal x\\n")
    except SyntaxError as exc:
        return exc.msg
    return None


results["class_nonlocal_syntaxerror"] = nonlocal_in_class_body_error()
