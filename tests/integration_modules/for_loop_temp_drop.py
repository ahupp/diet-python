
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


# diet-python: validate

def validate_module(module):
    assert module.run() is True
