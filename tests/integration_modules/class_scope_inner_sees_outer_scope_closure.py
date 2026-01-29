
def inner_sees_outer_scope_closure():
    z2 = "outer"

    class InnerSeesOuterScopeClosure:
        z2 = "inner"

        class InnerClosure:
            y = z2

    return InnerSeesOuterScopeClosure.InnerClosure.y


result = inner_sees_outer_scope_closure()


# diet-python: validate


module = __import__("sys").modules[__name__]
assert module.result == "outer"
