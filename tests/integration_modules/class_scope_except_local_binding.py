class Box:
    try:
        raise Exception("boom")
    except Exception as caught:
        seen = str(caught)


result = Box.seen
cleared = not hasattr(Box, "caught")


# diet-python: validate


module = __import__("sys").modules[__name__]
assert module.result == "boom"
assert module.cleared is True
