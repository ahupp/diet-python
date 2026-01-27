from pathlib import Path

from tests._integration import transformed_module


def test_method_named_open_uses_builtin(tmp_path):
    source = """
from pathlib import Path


class Wrapper:
    def __init__(self, path: Path) -> None:
        self.path = path

    def open(self, mode: str = 'r', *, encoding: str = 'utf8'):
        path = self.path
        return open(path, mode, encoding=encoding)


def write_and_read(path: Path) -> str:
    wrapper = Wrapper(path)
    with wrapper.open('w', encoding='utf8') as handle:
        handle.write('payload')
    with wrapper.open('r', encoding='utf8') as handle:
        return handle.read()
"""
    with transformed_module(tmp_path, "method_named_open", source) as module:
        target = tmp_path / "example.txt"
        assert module.write_and_read(target) == "payload"
