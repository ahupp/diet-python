from __future__ import annotations


def exercise():
    class LacksEnter:
        def __exit__(self, *exc_info):
            pass

    class LacksExit:
        def __enter__(self):
            return self

    errors = []
    for ctx in (LacksEnter(), LacksExit()):
        try:
            with ctx:
                pass
        except Exception as exc:
            errors.append((type(exc), str(exc)))
        else:
            errors.append(None)

    return errors

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
errors = module.exercise()
assert errors[0][0] is TypeError
assert "context manager" in errors[0][1]
assert errors[1][0] is TypeError
assert "__exit__" in errors[1][1]
