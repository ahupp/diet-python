from tests._integration import transformed_module
import pytest


def test_sync_generator_throw_handles_except_name_cleanup(tmp_path):
    source = """
import pytest


def make_gen():
    outer_capture = 2

    def gen():
        total = 1
        try:
            total += outer_capture
            yield total
        except ValueError as exc:
            total += len(str(exc))
        yield total

    return gen()


def exercise():
    gen_obj = make_gen()
    first = next(gen_obj)
    second = gen_obj.throw(ValueError("boom"))
    with pytest.raises(StopIteration):
        next(gen_obj)
    return first, second
"""

    with transformed_module(tmp_path, "sync_generator_throw_cleanup", source) as module:
        gen_obj = module.make_gen()
        assert next(gen_obj) == 3
        assert gen_obj.throw(ValueError("boom")) == 7
        with pytest.raises(StopIteration):
            next(gen_obj)
        assert module.exercise() == (3, 7)
