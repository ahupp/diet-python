from tests._integration import transformed_module
import pytest


def test_simple_sync_generator_stops_after_final_yield(tmp_path):
    source = """
def make_counter(delta):
    outer_capture = delta

    def gen():
        total = 1
        total += outer_capture
        sent = yield total
        total += sent
        yield total

    return gen()
"""
    with transformed_module(tmp_path, "simple_sync_generator_stop", source) as module:
        counter = module.make_counter(3)
        assert next(counter) == 4
        assert counter.send(5) == 9
        with pytest.raises(StopIteration):
            next(counter)
