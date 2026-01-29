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

# diet-python: validate

module = __import__("sys").modules[__name__]
func_locals = module.function_locals()
assert "h" in func_locals
assert "_dp_fn_h" not in func_locals
del func_locals["h"]
assert func_locals == {"x": 2, "y": 7, "w": 6}

class_locals = set(module.class_locals())
assert "x" not in class_locals
assert "y" in class_locals

assert module.class_namespace_overrides_closure() == 43
