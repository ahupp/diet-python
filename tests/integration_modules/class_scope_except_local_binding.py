class Box:
    try:
        raise Exception("boom")
    except Exception as caught:
        seen = str(caught)


result = Box.seen
cleared = not hasattr(Box, "caught")

# diet-python: validate

def validate_module(module):
    assert module.result == "boom"
    assert module.cleared is True
