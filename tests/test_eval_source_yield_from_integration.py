from __future__ import annotations

def test_eval_source_handles_yield_from(run_integration_module) -> None:
    with run_integration_module("eval_source_yield_from") as module:
        gen = module.make_values()
        forwarded = module.forward(gen)
        assert list(forwarded) == [1, 2, 3]
