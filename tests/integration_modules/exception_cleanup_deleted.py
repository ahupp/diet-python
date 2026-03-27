def cleanup_deleted():
    try:
        raise Exception()
    except Exception as e:
        del e
    try:
        e
    except UnboundLocalError:
        return False
    return True


def unbound_after_delete():
    try:
        raise Exception()
    except Exception as e:
        del e
    try:
        e
    except UnboundLocalError:
        return True
    return False

# diet-python: validate

def validate_module(module):
    assert module.cleanup_deleted() is False
    assert module.unbound_after_delete() is True
