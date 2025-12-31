import asyncio
from asyncio import futures


class SimpleEvilEventLoop(asyncio.base_events.BaseEventLoop):
    def get_debug(self):
        return False

    def __del__(self):
        if not self.is_closed() and not self.is_running():
            self.close()


def trigger():
    if not hasattr(futures, "_CFuture"):
        return "no_cfuture"

    called_on_fut_callback0 = False

    def pad():
        return ...

    def evil_call_soon(*_args, **_kwargs):
        nonlocal called_on_fut_callback0
        if called_on_fut_callback0:
            fut.remove_done_callback(int)
            fut.remove_done_callback(pad)
        else:
            called_on_fut_callback0 = True

    fake_event_loop = SimpleEvilEventLoop()
    fake_event_loop.call_soon = evil_call_soon

    fut = futures._CFuture(loop=fake_event_loop)
    fut.add_done_callback(str)
    fut.add_done_callback(int)
    fut.add_done_callback(pad)
    fut.add_done_callback(pad)
    fut.set_result("boom")
    return fut._callbacks


RESULT = trigger()
