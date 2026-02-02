from __future__ import annotations


def outer_read():
    x = 5

    def inner():
        return x

    return inner


def outer_assign_local():
    x = 5

    def inner():
        x = 2
        return x

    return inner


def outer_assign_local_read_before():
    x = 5

    def inner():
        return x
        x = 2

    return inner


def outer_nonlocal():
    x = 5

    def inner():
        nonlocal x
        x = 2
        return x

    return inner


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
inner = module.outer_read()
assert inner() == 5
assert inner.__closure__ is not None
# TODO: ignore __closure__ cell_contents for explicit cell lowering

inner = module.outer_assign_local()
assert inner() == 2
assert inner.__closure__ is None

inner = module.outer_assign_local_read_before()
assert inner.__closure__ is None
try:
    inner()
except UnboundLocalError:
    pass
else:
    raise AssertionError("expected UnboundLocalError")

inner = module.outer_nonlocal()
assert inner() == 2
assert inner.__closure__ is not None
# TODO: ignore __closure__ cell_contents for explicit cell lowering
