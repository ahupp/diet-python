from tests._integration import transformed_module


def test_for_loop_temp_drop_does_not_hold_references(tmp_path):
    source = """
import gc
import weakref

class C:
    pass

class Box:
    def __init__(self):
        self.obj = C()
    def __iter__(self):
        return self
    def __next__(self):
        if self.obj is None:
            raise StopIteration
        value = self.obj
        self.obj = None
        return value

def run():
    box = Box()
    ref = weakref.ref(box.obj)
    for item in box:
        del item
        gc.collect()
        return ref() is None
"""
    with transformed_module(tmp_path, "for_loop_temp_drop", source) as module:
        assert module.run() is True
