from __future__ import annotations


def test_method_name_local_binding_uses_local(run_integration_module):
    with run_integration_module("method_name_local_binding") as module:
        instance = module.Example()
        assert instance.close() == "ok"
