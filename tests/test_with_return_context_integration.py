from __future__ import annotations

import pytest


@pytest.mark.integration
def test_context_manager_exit_runs_on_return(run_integration_module):
    with run_integration_module("with_return_context") as module:
        exited, result = module.run()

    assert exited is True
    assert isinstance(result, module.Recording)
