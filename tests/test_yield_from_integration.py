from __future__ import annotations

import importlib
import sys
from pathlib import Path
from types import ModuleType

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

import diet_import_hook


def _import_module(module_name: str, module_path: Path) -> ModuleType:
    diet_import_hook.install()
    module_dir = str(module_path.parent)
    sys.path.insert(0, module_dir)
    try:
        if module_name in sys.modules:
            del sys.modules[module_name]
        return importlib.import_module(module_name)
    finally:
        if module_dir in sys.path:
            sys.path.remove(module_dir)


def test_yield_from_delegation(tmp_path):
    module_path = tmp_path / "yield_from_module.py"
    module_path.write_text(
        """

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
""",
        encoding="utf-8",
    )

    module = _import_module("yield_from_module", module_path)

    try:
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
    finally:
        if "yield_from_module" in sys.modules:
            del sys.modules["yield_from_module"]
