
import weakref
import gc

_ref = None

class Marker:
    pass

class CM:
    def __enter__(self):
        global _ref
        marker = Marker()
        _ref = weakref.ref(marker)
        return marker

    def __exit__(self, exc_type, exc, tb):
        return False


def run():
    with CM():
        pass
    gc.collect()
    return _ref() is None


# diet-python: validate

def validate_module(module):
    assert module.run() is True
