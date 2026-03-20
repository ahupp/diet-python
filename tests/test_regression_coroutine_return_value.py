from tests._integration import transformed_module


def test_coroutine_return_value_preserved(tmp_path):
    source = """
import asyncio

async def run():
    return 1

def main():
    return asyncio.run(run())

def manual():
    coro = run()
    try:
        coro.send(None)
    except StopIteration as exc:
        return exc.value
    raise AssertionError("expected StopIteration")
"""

    with transformed_module(tmp_path, "coroutine_return_value", source) as module:
        assert module.main() == 1
        assert module.manual() == 1


def test_direct_dunder_await_materializes_iterator(tmp_path):
    source = """
class Once:
    def __await__(self):
        if False:
            yield None
        return 42

def direct():
    it = Once().__await__()
    try:
        it.send(None)
    except StopIteration as exc:
        return exc.value
    raise AssertionError("expected StopIteration")
"""

    with transformed_module(tmp_path, "coroutine_dunder_await_direct", source) as module:
        assert module.direct() == 42


def test_closure_backed_coroutine_persists_state_across_send(tmp_path):
    source = """
class Once:
    def __await__(self):
        yield "tick"
        return 4

def make_runner(base):
    async def run():
        total = base
        total += await Once()
        return total
    return run

def manual():
    coro = make_runner(3)()
    assert coro.send(None) == "tick"
    try:
        coro.send(None)
    except StopIteration as exc:
        return exc.value
    raise AssertionError("expected StopIteration")
"""

    with transformed_module(tmp_path, "closure_backed_coroutine_persistence", source) as module:
        assert module.manual() == 7
