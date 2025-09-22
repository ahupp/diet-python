from __future__ import annotations


def test_comprehension_scope_shadowing(run_integration_module):
    with run_integration_module("comprehension_scope_shadowing") as module:
        assert module.HIGH_SCOPES == [module.Scope.Function]
        assert module.FUNCTION_MEMBERS == [module.Scope.Function]
