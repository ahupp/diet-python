from __future__ import annotations

import pytest
import __dp__

from tests._integration import integration_module


def test_eval_jit_mode_rejects_nested_def_and_generator_without_fallback(
    tmp_path, monkeypatch
):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
def outer(x):
    def inner(y):
        return y + 1
    def gen():
        yield inner(x)
        yield inner(x + 1)
    return list(gen())
"""
    with integration_module(tmp_path, "eval_jit_nested_gen_ok", source, mode="eval") as module:
        with pytest.raises(
            RuntimeError, match="requires fully lowered fastpath blocks"
        ):
            module.outer(2)


def test_eval_jit_mode_falls_back_to_original_code_for_coroutines(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
import asyncio

async def run():
    return 1

def run_sync():
    return asyncio.run(run())
"""
    with integration_module(tmp_path, "eval_jit_coroutine_fallback", source, mode="eval") as module:
        assert module.run_sync() == 1


def test_eval_jit_mode_invokes_jit_run_bb_plan(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
def add1(x):
    return x + 1
"""
    with integration_module(tmp_path, "eval_jit_invokes_run_bb", source, mode="eval") as module:
        original = __dp__._jit_run_bb_plan
        assert original is not None
        calls = {"count": 0}

        def wrapped(plan_module, plan_qualname, globals_dict, args):
            calls["count"] += 1
            return original(plan_module, plan_qualname, globals_dict, args)

        __dp__._jit_run_bb_plan = wrapped
        try:
            assert module.add1(2) == 3
        finally:
            __dp__._jit_run_bb_plan = original

        # CLIF-wrapper execution now dispatches directly from eval-frame using
        # code-extra metadata, so wrapper calls no longer flow through
        # __dp__._jit_run_bb_plan.
        assert calls["count"] == 0


def test_eval_jit_mode_module_init_binds_globals_to_module_dict(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
__dp_jit_module_init_sentinel = 123
"""
    with integration_module(tmp_path, "eval_jit_module_init_globals", source, mode="eval") as module:
        assert module.__dp_jit_module_init_sentinel == 123

        import __main__

        assert "__dp_jit_module_init_sentinel" not in __main__.__dict__
