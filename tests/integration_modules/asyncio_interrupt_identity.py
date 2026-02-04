import _thread
import asyncio
import operator


class _TaskWrapper:
    def __init__(self, loop, coro, **kwargs):
        self._task = asyncio.Task(coro, loop=loop, **kwargs)

    def cancel(self, *args, **kwargs):
        return self._task.cancel(*args, **kwargs)

    def add_done_callback(self, *args, **kwargs):
        return self._task.add_done_callback(*args, **kwargs)

    def remove_done_callback(self, *args, **kwargs):
        return self._task.remove_done_callback(*args, **kwargs)

    @property
    def _asyncio_future_blocking(self):
        return self._task._asyncio_future_blocking

    def result(self, *args, **kwargs):
        return self._task.result(*args, **kwargs)

    def done(self, *args, **kwargs):
        return self._task.done(*args, **kwargs)

    def cancelled(self, *args, **kwargs):
        return self._task.cancelled(*args, **kwargs)

    def exception(self, *args, **kwargs):
        return self._task.exception(*args, **kwargs)

    def get_loop(self, *args, **kwargs):
        return self._task.get_loop(*args, **kwargs)

    def set_name(self, *args, **kwargs):
        return self._task.set_name(*args, **kwargs)


class _LoopPolicy(asyncio.events._AbstractEventLoopPolicy):
    def __init__(self, loop_factory):
        self._loop_factory = loop_factory
        self.loop = None

    def get_event_loop(self):
        raise RuntimeError

    def new_event_loop(self):
        return self._loop_factory()

    def set_event_loop(self, loop):
        if loop is not None:
            self.loop = loop


def _new_loop():
    loop = asyncio.BaseEventLoop()
    loop._process_events = lambda *args, **kwargs: None
    loop._write_to_self = lambda *args, **kwargs: None

    class _Selector:
        def select(self, *args, **kwargs):
            return ()

    loop._selector = _Selector()

    async def shutdown_asyncgens():
        return None

    loop.shutdown_asyncgens = shutdown_asyncgens
    loop.set_task_factory(_TaskWrapper)
    return loop


def run_interrupt_case(iterations=200_000):
    async def main():
        _thread.interrupt_main()
        marker = object()
        for _ in range(iterations):
            if marker is marker:
                pass
        await asyncio.Event().wait()

    asyncio.events._set_event_loop_policy(_LoopPolicy(_new_loop))
    try:
        asyncio.run(main())
    except BaseException as exc:
        return exc
    finally:
        asyncio.events._set_event_loop_policy(None)
    raise AssertionError("expected exception")


# diet-python: validate
import asyncio
import builtins
import operator

if __dp_integration_mode__ != "stock":
    assert builtins.__dp__.is_ is operator.is_
    assert builtins.__dp__.is_not is operator.is_not

exc = run_interrupt_case()
assert isinstance(exc, asyncio.CancelledError), type(exc)
