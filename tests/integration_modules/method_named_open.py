from pathlib import Path


class Wrapper:
    def __init__(self, path: Path) -> None:
        self.path = path

    def open(self, mode: str = "r", *, encoding: str = "utf8"):
        path = self.path
        return open(path, mode, encoding=encoding)


def write_and_read(path: Path) -> str:
    wrapper = Wrapper(path)
    with wrapper.open("w", encoding="utf8") as handle:
        handle.write("payload")
    with wrapper.open("r", encoding="utf8") as handle:
        return handle.read()


# diet-python: validate

def validate_module(module):
    import tempfile
    from pathlib import Path

    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "example.txt"
        assert module.write_and_read(path) == "payload"
