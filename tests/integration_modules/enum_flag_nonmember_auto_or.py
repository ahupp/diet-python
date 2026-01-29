from __future__ import annotations

from enum import Flag, auto, nonmember


class Status(Flag):
    A = auto()
    B = auto()
    ALL = nonmember(A | B)


def build_values():
    return Status.A, Status.B, Status.ALL

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
a, b, all_value = module.build_values()
assert a.value == 1
assert b.value == 2
assert all_value == 3
