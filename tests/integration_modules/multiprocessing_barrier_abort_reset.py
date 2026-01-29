import multiprocessing as mp
import queue
import threading


def _participant(barrier, barrier2, result_queue):
    try:
        index = barrier.wait()
        if index == 1:
            raise RuntimeError("boom")
        barrier.wait()
    except threading.BrokenBarrierError:
        result_queue.put("broken")
    except RuntimeError:
        barrier.abort()

    try:
        if barrier2.wait() == 1:
            barrier.reset()
        barrier2.wait()
        barrier.wait()
        result_queue.put("done")
    except threading.BrokenBarrierError:
        result_queue.put("barrier2_broken")


def run() -> bool:
    ctx = mp.get_context("spawn")
    barrier = ctx.Barrier(3, timeout=2.0)
    barrier2 = ctx.Barrier(3, timeout=2.0)
    result_queue = ctx.Queue()

    processes = [
        ctx.Process(target=_participant, args=(barrier, barrier2, result_queue))
        for _ in range(2)
    ]
    for proc in processes:
        proc.start()

    _participant(barrier, barrier2, result_queue)

    for proc in processes:
        proc.join(5)

    if any(proc.is_alive() for proc in processes):
        for proc in processes:
            proc.terminate()
        return False

    results = []
    while len(results) < 5:
        try:
            results.append(result_queue.get(timeout=1))
        except queue.Empty:
            break

    return results.count("done") == 3 and "barrier2_broken" not in results

# diet-python: validate

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

module = __import__("sys").modules[__name__]
if not _supports_multiprocessing_barrier():
    pytest.skip("multiprocessing barrier not available in this environment")
assert module.run() is True
