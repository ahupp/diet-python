def cleanup_deleted():
    try:
        raise Exception()
    except Exception as e:
        del e
    return "e" in locals()


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
