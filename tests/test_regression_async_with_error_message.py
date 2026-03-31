import pytest

def test_async_with_aenter_error_message(run_integration_module):
    with run_integration_module("bad_async_enter") as module:
        with pytest.raises(
            TypeError,
            match=r"'async with' received an object from __aenter__ that does not implement __await__: int",
        ):
            module.main()


def test_async_with_aexit_error_message(run_integration_module):
    with run_integration_module("bad_async_exit") as module:
        with pytest.raises(
            TypeError,
            match=r"'async with' received an object from __aexit__ that does not implement __await__: int",
        ):
            module.main()
