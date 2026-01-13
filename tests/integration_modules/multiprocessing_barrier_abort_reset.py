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
