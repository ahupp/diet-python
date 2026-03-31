import pytest

def test_if_empty_body_preserves_truthiness(run_integration_module):
    with run_integration_module("truthiness_notimplemented") as module:
        with pytest.raises(TypeError, match="NotImplemented should not be used in a boolean context"):
            module.run()
