
def nonlocal_in_class_body_error():
    try:
        exec("class Bad:\n    nonlocal x\n")
    except SyntaxError as exc:
        return exc.msg
    return None


result = nonlocal_in_class_body_error()


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result is not None
assert module.result
