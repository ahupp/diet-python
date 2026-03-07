import pytest


def make_counter(delta):
    outer_capture = delta

    def gen():
        total = 1
        total += outer_capture
        sent = yield total
        total += sent
        yield total

    return gen()


def exercise_throw():
    outer_capture = 2

    def gen():
        total = 1
        try:
            total += outer_capture
            yield total
        except ValueError as exc:
            total += len(str(exc))
        yield total

    gen_obj = gen()
    first = next(gen_obj)
    second = gen_obj.throw(ValueError("boom"))
    with pytest.raises(StopIteration):
        next(gen_obj)
    return first, second


def exercise_yield_from():
    outer_capture = 3

    def child():
        total = 1
        total += outer_capture
        yield total
        total += 10
        yield total
        return total

    def delegator():
        final = yield from child()
        yield final + 100

    gen_obj = delegator()
    first = next(gen_obj)
    second = next(gen_obj)
    third = next(gen_obj)
    with pytest.raises(StopIteration):
        next(gen_obj)
    return first, second, third


# diet-python: validate

module = __import__("sys").modules[__name__]

counter = module.make_counter(3)
assert next(counter) == 4
assert counter.send(5) == 9
with pytest.raises(StopIteration):
    next(counter)

assert module.exercise_throw() == (3, 7)
assert module.exercise_yield_from() == (4, 14, 114)
