import asyncio
import inspect
import types

import __dp__


def _build_wrapped(names, is_async, is_generator, entry):
    code = __dp__.code_with_freevars(names, is_async, is_generator)
    closure = tuple(__dp__.make_cell(f"cell-{name}") for name in names)
    wrapped = types.FunctionType(code, globals(), name="wrapped", closure=closure)
    wrapped.__kwdefaults__ = {"__dp_entry": entry}
    return wrapped


def test_code_with_freevars_returns_requested_freevars():
    code = __dp__.code_with_freevars(("x", "y"), False, False)
    assert code.co_freevars == ("x", "y")


def test_bb_wrap_with_named_closure_reorders_cells_to_code_freevars():
    def entry():
        return None

    wrapped = __dp__._bb_wrap_with_named_closure(
        entry,
        ("a", "_dp_eval_1", "_dp_pc"),
        ("captured-a", "captured-eval", "captured-pc"),
    )

    assert wrapped.__code__.co_freevars == ("_dp_eval_1", "_dp_pc", "a")
    assert {
        name: cell.cell_contents
        for name, cell in zip(wrapped.__code__.co_freevars, wrapped.__closure__)
    } == {
        "_dp_eval_1": "captured-eval",
        "_dp_pc": "captured-pc",
        "a": "captured-a",
    }


def test_code_with_freevars_builds_sync_wrapper():
    def entry(*args, **kwargs):
        return (args, kwargs)

    wrapped = _build_wrapped(("x", "y"), False, False, entry)

    assert wrapped(1, value=2) == ((1,), {"value": 2})


def test_code_with_freevars_builds_coroutine_wrapper():
    async def entry(*args, **kwargs):
        return (args, kwargs)

    wrapped = _build_wrapped(("x",), True, False, entry)

    assert inspect.iscoroutinefunction(wrapped)
    assert asyncio.run(wrapped(1, value=2)) == ((1,), {"value": 2})


def test_code_with_freevars_builds_generator_wrapper():
    def entry(*args, **kwargs):
        yield args
        yield kwargs

    wrapped = _build_wrapped(("x",), False, True, entry)

    assert inspect.isgeneratorfunction(wrapped)
    assert list(wrapped(1, value=2)) == [(1,), {"value": 2}]


def test_code_with_freevars_builds_async_generator_wrapper():
    async def entry(*args, **kwargs):
        yield args
        yield kwargs

    wrapped = _build_wrapped(("x",), True, True, entry)

    async def collect():
        return [item async for item in wrapped(1, value=2)]

    assert inspect.isasyncgenfunction(wrapped)
    assert asyncio.run(collect()) == [(1,), {"value": 2}]
