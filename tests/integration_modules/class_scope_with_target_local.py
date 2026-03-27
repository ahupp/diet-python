from contextlib import nullcontext


class Box:
    with nullcontext("ok") as value:
        seen = value


result = (Box.value, Box.seen)

# diet-python: validate

def validate_module(module):
    assert module.result == ("ok", "ok")
