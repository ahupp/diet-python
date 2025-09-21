from __future__ import annotations

from ._integration import transformed_module


MODULE_SOURCE = """
def bounded_loop(limit=1):
    start = 0
    while start <= limit:
        start += 1
        if start > 2:
            raise RuntimeError("loop guard not recomputed")
    return start
"""


def test_while_condition_recomputed_each_iteration(tmp_path):
    with transformed_module(tmp_path, "bounded_loop", MODULE_SOURCE) as module:
        assert module.bounded_loop() == 2
