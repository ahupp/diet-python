from __future__ import annotations

def test_bare_except_does_not_shadow_module_globals(run_integration_module):
    with run_integration_module("translation_module") as module:
        assert module.call_translate() == "translated:after except"
