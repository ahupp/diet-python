def make_counter(delta):
    outer_capture = delta

    def gen():
        total = 1
        total += outer_capture
        sent = yield total
        total += sent
        yield total

    return gen()


# diet-python: validate

def validate_module(module):
    import pytest

    counter = module.make_counter(3)
    assert next(counter) == 4
    assert counter.send(5) == 9
    with pytest.raises(StopIteration):
        next(counter)
