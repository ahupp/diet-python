from tests._integration import transformed_module


def test_with_enter_result_is_not_retained(tmp_path):
    source = """
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
"""
    with transformed_module(tmp_path, "with_enter_result_lifetime", source) as module:
        assert module.run() is True
