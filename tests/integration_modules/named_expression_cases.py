def dict_comp_fib():
    a, b = 1, 2
    fib = {(c := a): (a := b) + (b := a + c) - b for __ in range(6)}
    return fib


def genexp_scope_state():
    a = 1
    b = [1, 2, 3, 4]
    genexp = (c := i + a for i in b)
    has_c_before = "c" in locals()
    values = list(genexp)
    return has_c_before, values, c


def mangled_global_value():
    class Foo:
        def f(self_):
            global __x1
            __x1 = 0
            [_Foo__x1 := 1 for a in [2]]
            [__x1 := 2 for a in [3]]

    Foo().f()
    return _Foo__x1
