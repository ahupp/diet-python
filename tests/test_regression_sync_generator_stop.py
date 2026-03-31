import pytest


def test_simple_sync_generator_stops_after_final_yield(run_integration_module):
    with run_integration_module("simple_sync_generator_stop") as module:
        counter = module.make_counter(3)
        assert next(counter) == 4
        assert counter.send(5) == 9
        with pytest.raises(StopIteration):
            next(counter)
