from __future__ import annotations

import pytest

def test_yield_from_delegation(run_integration_module):
    with run_integration_module("yield_from_module") as module:
        assert "__dp__" in module.__dict__

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
