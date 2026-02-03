from tests._integration import transformed_module


def test_unpack_temp_drop_does_not_hold_references(tmp_path):
    source = """
import gc
import weakref

class C:
    pass

def run():
    obj = C()
    ref = weakref.ref(obj)
    a, b = (obj, 1)
    del obj
    del a
    gc.collect()
    return ref() is None
"""
    with transformed_module(tmp_path, "unpack_temp_drop", source) as module:
        assert module.run() is True
