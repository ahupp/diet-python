from __future__ import annotations

def test_class_body_uses_globals_when_method_shares_name(run_integration_module):
    with run_integration_module("method_name_clash") as module:
        instance = module.Example()
        result = instance.date()
        assert isinstance(result, module.date)

