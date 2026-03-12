from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

DP = __import__("__dp__")


@pytest.fixture(autouse=True)
def restore_dp_hooks(monkeypatch):
    monkeypatch.setattr(DP, "_register_clif_vectorcall", None)
    monkeypatch.setattr(DP, "_jit_compile_clif_wrapper", None)


def test_lazy_compile_mode_does_not_eager_compile(monkeypatch):
    calls = []

    def register(entry, module_name, function_id, metadata):
        calls.append(("register", entry, module_name, function_id, metadata))

    def eager_compile(entry):
        calls.append(("compile", entry))

    monkeypatch.setattr(DP, "_register_clif_vectorcall", register)
    monkeypatch.setattr(DP, "_jit_compile_clif_wrapper", eager_compile)
    monkeypatch.delenv("DIET_PYTHON_JIT_COMPILE_MODE", raising=False)

    entry = lambda: None
    DP._bb_enable_lazy_clif_vectorcall(
        entry,
        "m",
        0,
        "q",
        ("x",),
        None,
        {},
        None,
        object(),
        object(),
        DP._BIND_KIND_FUNCTION,
    )

    assert [call[0] for call in calls] == ["register"]


def test_eager_compile_mode_eager_compiles(monkeypatch):
    calls = []

    def register(entry, module_name, function_id, metadata):
        calls.append(("register", entry, module_name, function_id, metadata))

    def eager_compile(entry):
        calls.append(("compile", entry))

    monkeypatch.setattr(DP, "_register_clif_vectorcall", register)
    monkeypatch.setattr(DP, "_jit_compile_clif_wrapper", eager_compile)
    monkeypatch.setenv("DIET_PYTHON_JIT_COMPILE_MODE", "eager")

    entry = lambda: None
    DP._bb_enable_lazy_clif_vectorcall(
        entry,
        "m",
        0,
        "q",
        ("x",),
        None,
        {},
        None,
        object(),
        object(),
        DP._BIND_KIND_FUNCTION,
    )

    assert [call[0] for call in calls] == ["register", "compile"]
