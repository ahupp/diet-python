import pytest


@pytest.mark.integration
def test_unpacking_iterator_succeeds(run_integration_module):
    with run_integration_module("map_unpack") as module:
        assert module.RESULT[0] == "ok"
        assert module.RESULT[1] == (1, 2, 3)
