from __future__ import annotations

from enum import Enum


class Period(Enum):
    _ignore_ = "Period i"
    Period = vars()
    for i in range(2):
        Period[f"day_{i}"] = i
    OneDay = day_1
