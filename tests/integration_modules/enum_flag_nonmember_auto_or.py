from __future__ import annotations

from enum import Flag, auto, nonmember


class Status(Flag):
    A = auto()
    B = auto()
    ALL = nonmember(A | B)


def build_values():
    return Status.A, Status.B, Status.ALL
