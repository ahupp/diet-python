from contextlib import nullcontext


class Box:
    with nullcontext("ok") as value:
        seen = value


result = (Box.value, Box.seen)


# diet-python: validate


module = __import__("sys").modules[__name__]
assert module.result == ("ok", "ok")
