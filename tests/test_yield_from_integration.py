from __future__ import annotations

import pytest

from ._integration import transformed_module


MODULE_SOURCE = """
def child():
    events = []
    try:
        value = yield "start"
        events.append(("send", value))
        while True:
            try:
                value = yield value
                events.append(("send", value))
            except KeyError as exc:
                events.append(("throw", str(exc)))
                value = "handled"
            if value == "stop":
                break
    finally:
        events.append(("finally", None))
    return events


def delegator():
    result = yield from child()
    return ("done", result)
"""


def test_yield_from_delegation(tmp_path):
    with transformed_module(tmp_path, "yield_from_module", MODULE_SOURCE) as module:
        assert "__dp__" in module.delegator.__code__.co_names

        gen = module.delegator()

        assert next(gen) == "start"
        assert gen.send("first") == "first"
        assert gen.throw(KeyError("boom")) == "handled"

        with pytest.raises(StopIteration) as exc:
            gen.send("stop")

        result = exc.value.value
        assert result[0] == "done"
        assert result[1] == [
            ("send", "first"),
            ("throw", "'boom'"),
            ("send", "stop"),
            ("finally", None),
        ]
