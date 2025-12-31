from __future__ import annotations

import pytest


@pytest.mark.integration
def test_with_class_mock_calls(run_integration_module):
    with run_integration_module("with_class_mock_calls") as module:
        enter_calls, exit_calls = module.run()

    assert enter_calls == [module.mock.call()]
    assert exit_calls == [module.mock.call(None, None, None)]
