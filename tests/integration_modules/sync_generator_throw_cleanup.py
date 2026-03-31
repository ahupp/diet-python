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


# diet-python: validate

def validate_module(module):
    import pytest

    gen_obj = module.make_gen()
    assert next(gen_obj) == 3
    assert gen_obj.throw(ValueError("boom")) == 7
    with pytest.raises(StopIteration):
        next(gen_obj)
    assert module.exercise() == (3, 7)
