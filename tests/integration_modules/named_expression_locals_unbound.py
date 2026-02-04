def named_expr_locals_state():
    a = 1
    b = [1]
    genexp = (c := i + a for i in b)
    has_c_before = "c" in locals()
    values = list(genexp)
    return has_c_before, values, c

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.named_expr_locals_state() == (False, [2], 2)
