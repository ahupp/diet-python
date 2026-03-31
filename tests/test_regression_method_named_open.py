from pathlib import Path

def test_method_named_open_uses_builtin(tmp_path: Path, run_integration_module):
    with run_integration_module("method_named_open") as module:
        target = tmp_path / "example.txt"
        assert module.write_and_read(target) == "payload"
