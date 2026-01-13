from __future__ import annotations

import multiprocessing as mp
import queue
import threading

import pytest


def _barrier_smoke(barrier, result_queue) -> None:
    try:
        barrier.wait()
        result_queue.put("ok")
    except BaseException as exc:
        result_queue.put(repr(exc))


def _supports_multiprocessing_barrier() -> bool:
    try:
        ctx = mp.get_context("spawn")
        barrier = ctx.Barrier(2, timeout=1.0)
        result_queue = ctx.Queue()
    except (OSError, PermissionError, RuntimeError):
        return False
    proc = ctx.Process(target=_barrier_smoke, args=(barrier, result_queue))
    proc.start()
    try:
        barrier.wait()
    except threading.BrokenBarrierError:
        proc.terminate()
        proc.join(1)
        return False
    proc.join(2)
    if proc.is_alive():
        proc.terminate()
        proc.join(1)
        return False
    try:
        result = result_queue.get_nowait()
    except queue.Empty:
        return False
    return result == "ok"


@pytest.mark.integration
def test_multiprocessing_barrier_abort_reset(run_integration_module):
    if not _supports_multiprocessing_barrier():
        pytest.skip("multiprocessing barrier not available in this environment")
    with run_integration_module("multiprocessing_barrier_abort_reset") as module:
        assert module.run() is True
