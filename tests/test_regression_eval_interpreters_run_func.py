import pytest

from tests._integration import integration_module


pytest.importorskip("_interpreters")


def test_eval_run_func_validates_shared_keys_for_soac_function(tmp_path):
    pytest.xfail(
        "requires __dp__ _patch_interpreters compatibility for _interpreters.run_func"
    )
    source = """
import _interpreters


def run():
    interp = _interpreters.create()
    try:
        def script():
            pass

        try:
            _interpreters.run_func(interp, script, shared={"\\ud82a": 0})
        except Exception as exc:
            return type(exc).__name__, str(exc)
    finally:
        _interpreters.destroy(interp)
"""
    with integration_module(tmp_path, "eval_interpreters_run_func", source, mode="eval") as module:
        err_name, err_msg = module.run()
        assert err_name == "UnicodeEncodeError"
        assert "surrogates not allowed" in err_msg
