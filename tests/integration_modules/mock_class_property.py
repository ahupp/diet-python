from unittest import mock


def mock_class_property_ok():
    m = mock.Mock(spec=int)
    return isinstance(m, int)


RESULT = mock_class_property_ok()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.RESULT is True
