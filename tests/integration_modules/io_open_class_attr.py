import io


class Reader:
    open = io.open


def read_self():
    with Reader().open(__file__, "rb") as handle:
        return handle.read(1)


RESULT = read_self()

# diet-python: validate

def validate_module(module):

    assert isinstance(module.RESULT, bytes)
    assert len(module.RESULT) == 1
