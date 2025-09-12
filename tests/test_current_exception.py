from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import __dp__


def test_current_exception_returns_exc():
    try:
        raise ValueError("oops")
    except ValueError:
        exc = __dp__.current_exception()
        assert isinstance(exc, ValueError)
        assert str(exc) == "oops"
    else:  # pragma: no cover
        assert False


def test_current_exception_no_exc():
    assert __dp__.current_exception() is None
