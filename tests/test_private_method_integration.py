import pytest


@pytest.mark.integration
def test_private_method_lookup(run_integration_module):
    with run_integration_module("private_method") as module:
        assert module.RESULT == "payload"
