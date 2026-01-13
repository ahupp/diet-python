def function_locals():
    def f(x):
        def g(y):
            def h(z):
                return y + z
            w = x + y
            y += 3
            return locals()
        return g

    return f(2)(4)


def class_locals():
    def f(x):
        class C:
            y = x
            def m(self):
                return x
            z = list(locals())
        return C

    return f(1).z


def class_namespace_overrides_closure():
    x = 42
    class X:
        locals()["x"] = 43
        y = x
    return X.y
