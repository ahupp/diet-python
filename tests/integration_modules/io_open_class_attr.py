import io


class Reader:
    open = io.open


def read_self():
    with Reader().open(__file__, "rb") as handle:
        return handle.read(1)


RESULT = read_self()
