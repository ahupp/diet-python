from __future__ import annotations

def test_method_local_shadowing_uses_local(run_integration_module):
    with run_integration_module("method_local_shadowing") as module:
        instance = module.Example()
        assert instance.run() == 1
