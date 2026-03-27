
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


# diet-python: validate

def validate_module(module):
    assert module.run() is True
