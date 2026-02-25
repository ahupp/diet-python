from __future__ import annotations

import pytest

from tests._integration import integration_module


def test_eval_jit_mode_allows_nested_def_and_generator(tmp_path, monkeypatch):
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
        assert module.outer(2) == [3, 4]


def test_eval_jit_mode_rejects_coroutines(tmp_path, monkeypatch):
    monkeypatch.setenv("DIET_PYTHON_JIT", "1")
    source = """
async def run():
    return 1
"""
    with pytest.raises(RuntimeError, match="coroutine"):
        with integration_module(
            tmp_path, "eval_jit_reject_coroutine", source, mode="eval"
        ):
            pass
