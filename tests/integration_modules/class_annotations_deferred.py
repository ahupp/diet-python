class ThemeSection:
    if False:
        from typing import ClassVar
    __dataclass_fields__: ClassVar[int]

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
with pytest.raises(NameError):
    _ = module.ThemeSection.__annotations__
