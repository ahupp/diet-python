from pathlib import Path
import tempfile


class Wrapper:
    def __init__(self, path: Path) -> None:
        self.path = path

    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        path = self.path
        return open(path, mode, encoding=encoding)


def write_and_read() -> str:
    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "example.txt"
        wrapper = Wrapper(path)
        with wrapper.open("w", encoding="utf8") as handle:
            handle.write("payload")
        with wrapper.open("r", encoding="utf8") as handle:
            return handle.read()


RESULT = write_and_read()

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.RESULT == "payload"
