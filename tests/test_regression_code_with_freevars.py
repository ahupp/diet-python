import asyncio
import inspect
import types

import __dp__


def _make_cell(value):
    cell = types.CellType()
    cell.cell_contents = value
    return cell


def _build_wrapped(names, is_async, is_generator, entry):
    code = __dp__.code_with_freevars(names, is_async, is_generator)
    closure = tuple(_make_cell(f"cell-{name}") for name in names)
    wrapped = types.FunctionType(code, globals(), name="wrapped", closure=closure)
    wrapped.__kwdefaults__ = {"__dp_entry": entry}
    return wrapped


def test_code_with_freevars_returns_requested_freevars():
    code = __dp__.code_with_freevars(("x", "y"), False, False)
    assert code.co_freevars == ("x", "y")


def test_code_with_freevars_uses_canonical_freevar_order():
    code = __dp__.code_with_freevars(("a", "_dp_eval_1", "_dp_pc"), False, False)

    assert code.co_freevars == ("_dp_eval_1", "_dp_pc", "a")

    captured_by_name = {
        "_dp_eval_1": "captured-eval",
        "_dp_pc": "captured-pc",
        "a": "captured-a",
    }
    closure = tuple(_make_cell(captured_by_name[name]) for name in code.co_freevars)
    wrapped = types.FunctionType(code, globals(), name="wrapped", closure=closure)
    wrapped.__kwdefaults__ = {"__dp_entry": lambda: None}

    assert {
        name: cell.cell_contents
        for name, cell in zip(wrapped.__code__.co_freevars, wrapped.__closure__)
    } == captured_by_name


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
