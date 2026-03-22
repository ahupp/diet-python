from __future__ import annotations


def has_exception_name():
    try:
        1 / 0
    except Exception as e:
        pass
    return "e" in locals()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
if __dp_integration_transformed__:
    try:
        module.has_exception_name()
    except NotImplementedError:
        pass
    else:
        raise AssertionError("expected locals() to be unsupported")
else:
    assert module.has_exception_name() is False
