def cleanup_global_exception_name():
    global exc
    exc = "old"
    try:
        raise Exception("boom")
    except Exception as exc:
        inside = ("exc" in globals(), exc.args[0])
    return inside, ("exc" in globals()), globals().get("exc", "<missing>")

# diet-python: validate

def validate_module(module):
    assert module.cleanup_global_exception_name() == ((True, "boom"), False, "<missing>")
