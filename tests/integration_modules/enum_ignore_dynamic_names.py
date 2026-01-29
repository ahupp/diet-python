from __future__ import annotations

from enum import Enum


class Period(Enum):
    _ignore_ = "Period i"
    Period = vars()
    for i in range(2):
        Period[f"day_{i}"] = i
    OneDay = day_1

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
period = module.Period
assert period.OneDay is period.day_1
assert period.OneDay.value == 1
