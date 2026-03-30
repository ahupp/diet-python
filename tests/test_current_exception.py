from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

from soac import runtime as DP


def test_current_exception_returns_exc():
    try:
        raise ValueError("oops")
    except ValueError:
        exc = DP.current_exception()
        assert isinstance(exc, ValueError)
        assert str(exc) == "oops"
    else:  # pragma: no cover
        assert False


def test_current_exception_no_exc():
    assert DP.current_exception() is None
