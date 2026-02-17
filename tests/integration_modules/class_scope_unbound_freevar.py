def outer():
    class C:
        y = x

    x = 1
    return C


# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
with pytest.raises(NameError, match="cannot access free variable"):
    module.outer()
