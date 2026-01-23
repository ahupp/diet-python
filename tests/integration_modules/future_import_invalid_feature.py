from __future__ import not_a_feature

VALUE = 1

# diet-python: validate

from __future__ import annotations

import pytest

def validate(module):
    with pytest.raises(SyntaxError) as excinfo:
        pass
    assert "not_a_feature" in str(excinfo.value)
