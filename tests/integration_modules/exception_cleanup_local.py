def cleanup_local():
    try:
        raise Exception("boom")
    except Exception as exc:
        inside = exc.args[0]
    try:
        exc
    except UnboundLocalError:
        return inside
    raise AssertionError("expected local except name to be deleted")


RESULT = cleanup_local()

# diet-python: validate

def validate_module(module):
    assert module.RESULT == "boom"
