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
