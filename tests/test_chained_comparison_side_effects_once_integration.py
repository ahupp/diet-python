from __future__ import annotations


def test_chained_comparison_side_effects_once(run_integration_module) -> None:
    with run_integration_module("chained_comparison_side_effects_once") as module:
        assert module.probe() == ["hit"]
