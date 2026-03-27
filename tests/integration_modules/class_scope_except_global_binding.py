class Box:
    global caught
    try:
        raise Exception("boom")
    except Exception as caught:
        seen = str(caught)


result = Box.seen
cleared = "caught" not in globals()

# diet-python: validate

def validate_module(module):
    assert module.result == "boom"
    assert module.cleared is True
